//! Minimal parser for Valve's text KeyValues format (`.vdf`, `.acf`).
//!
//! Supports quoted and bare tokens, nested `{ }` blocks, `//` comments and the
//! escape sequences Steam writes (`\\`, `\"`, `\n`, `\t`). Key lookups are
//! case-insensitive because Steam is not consistent (`apps` vs `Apps`).
//! Conditional tags such as `[$WIN32]` are skipped.

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, PartialEq)]
pub enum Vdf {
    Str(String),
    Obj(Vec<(String, Vdf)>),
}

impl Vdf {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Vdf::Str(s) => Some(s),
            Vdf::Obj(_) => None,
        }
    }

    pub fn entries(&self) -> &[(String, Vdf)] {
        match self {
            Vdf::Obj(entries) => entries,
            Vdf::Str(_) => &[],
        }
    }

    /// First child with this key (case-insensitive).
    pub fn get(&self, key: &str) -> Option<&Vdf> {
        self.entries()
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v)
    }

    /// Follows a path of keys, e.g. `["Software", "Valve", "Steam", "apps"]`.
    pub fn path(&self, keys: &[&str]) -> Option<&Vdf> {
        keys.iter().try_fold(self, |node, key| node.get(key))
    }

    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Vdf::as_str)
    }

    pub fn get_u64(&self, key: &str) -> Option<u64> {
        self.get_str(key).and_then(|s| s.trim().parse().ok())
    }
}

#[derive(Debug, PartialEq)]
enum Token {
    Text(String),
    Open,
    Close,
}

fn tokenize(input: &str) -> AppResult<Vec<Token>> {
    let mut tokens = Vec::new();
    let mut chars = input.trim_start_matches('\u{feff}').chars().peekable();
    while let Some(&c) = chars.peek() {
        match c {
            c if c.is_whitespace() => {
                chars.next();
            }
            '{' => {
                chars.next();
                tokens.push(Token::Open);
            }
            '}' => {
                chars.next();
                tokens.push(Token::Close);
            }
            '/' => {
                chars.next();
                if chars.peek() == Some(&'/') {
                    for c in chars.by_ref() {
                        if c == '\n' {
                            break;
                        }
                    }
                } else {
                    tokens.push(Token::Text(read_bare(&mut chars, "/")));
                }
            }
            '[' => {
                // Conditional like [$WIN32]: ignored.
                for c in chars.by_ref() {
                    if c == ']' {
                        break;
                    }
                }
            }
            '"' => {
                chars.next();
                let mut s = String::new();
                let mut closed = false;
                while let Some(c) = chars.next() {
                    match c {
                        '"' => {
                            closed = true;
                            break;
                        }
                        '\\' => match chars.next() {
                            Some('n') => s.push('\n'),
                            Some('t') => s.push('\t'),
                            Some(other) => s.push(other),
                            None => break,
                        },
                        other => s.push(other),
                    }
                }
                if !closed {
                    return Err(AppError::Invalid("vdf: unterminated string".into()));
                }
                tokens.push(Token::Text(s));
            }
            _ => tokens.push(Token::Text(read_bare(&mut chars, ""))),
        }
    }
    Ok(tokens)
}

fn read_bare(chars: &mut std::iter::Peekable<std::str::Chars<'_>>, prefix: &str) -> String {
    let mut s = prefix.to_string();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() || c == '{' || c == '}' || c == '"' {
            break;
        }
        s.push(c);
        chars.next();
    }
    s
}

fn parse_entries(
    tokens: &mut std::iter::Peekable<std::vec::IntoIter<Token>>,
    nested: bool,
) -> AppResult<Vec<(String, Vdf)>> {
    let mut entries = Vec::new();
    loop {
        match tokens.next() {
            None if nested => return Err(AppError::Invalid("vdf: missing '}'".into())),
            None => return Ok(entries),
            Some(Token::Close) if nested => return Ok(entries),
            Some(Token::Close) => return Err(AppError::Invalid("vdf: unexpected '}'".into())),
            Some(Token::Open) => return Err(AppError::Invalid("vdf: unexpected '{'".into())),
            Some(Token::Text(key)) => match tokens.next() {
                Some(Token::Text(value)) => entries.push((key, Vdf::Str(value))),
                Some(Token::Open) => entries.push((key, Vdf::Obj(parse_entries(tokens, true)?))),
                _ => return Err(AppError::Invalid(format!("vdf: key '{key}' has no value"))),
            },
        }
    }
}

/// Parses a whole document. The result is an object holding the top-level
/// entries (usually a single root key such as `"AppState"`).
pub fn parse(input: &str) -> AppResult<Vdf> {
    let mut tokens = tokenize(input)?.into_iter().peekable();
    Ok(Vdf::Obj(parse_entries(&mut tokens, false)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_blocks_and_escapes() {
        let doc = parse(
            r#"
            "libraryfolders"
            {
                "0"
                {
                    "path"      "C:\\Program Files (x86)\\Steam"
                    "label"     ""
                    "apps" { "730" "123" }
                }
            }
            "#,
        )
        .unwrap();
        let lib = doc.path(&["libraryfolders", "0"]).unwrap();
        assert_eq!(lib.get_str("path"), Some(r"C:\Program Files (x86)\Steam"));
        assert_eq!(lib.get_str("label"), Some(""));
        assert_eq!(lib.path(&["apps"]).unwrap().get_u64("730"), Some(123));
    }

    #[test]
    fn keys_are_case_insensitive_and_order_is_kept() {
        let doc = parse(r#""Root" { "Apps" { "b" "1" "a" "2" } }"#).unwrap();
        let apps = doc.path(&["root", "apps"]).unwrap();
        let keys: Vec<_> = apps.entries().iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, ["b", "a"]);
    }

    #[test]
    fn skips_comments_and_conditionals_and_bom() {
        let doc = parse("\u{feff}// header\n\"a\" \"1\" [$WIN32]\n\"b\" { // c\n }").unwrap();
        assert_eq!(doc.get_str("a"), Some("1"));
        assert_eq!(doc.get("b"), Some(&Vdf::Obj(vec![])));
    }

    #[test]
    fn accepts_bare_tokens() {
        let doc = parse("root { key value }").unwrap();
        assert_eq!(doc.path(&["root"]).unwrap().get_str("key"), Some("value"));
    }

    #[test]
    fn rejects_malformed_documents() {
        assert!(parse(r#""a" { "b" "c""#).is_err());
        assert!(parse(r#""a" "unterminated"#).is_err());
        assert!(parse(r#""a""#).is_err());
        assert!(parse("}").is_err());
    }

    #[test]
    fn missing_values_return_none() {
        let doc = parse(r#""a" { "n" "not a number" }"#).unwrap();
        let a = doc.get("a").unwrap();
        assert_eq!(a.get_u64("n"), None);
        assert_eq!(a.get_str("missing"), None);
        assert_eq!(a.as_str(), None);
    }
}
