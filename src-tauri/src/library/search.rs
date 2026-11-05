//! Search bar syntax → filter rules (the same rules as smart collections).
//!
//! `source:gog heures:>20 tag:souls statut:done -installe:oui zelda`
//!
//! - `key:value`, values with spaces in quotes (`tag:"souls like"`), `-` negates;
//! - numbers accept `>`, `<`, `>=`, `<=`, `=` or a range `10-20`;
//! - anything else is free text matched against the title.
//!
//! Unknown keys or values are kept as plain text and reported as warnings.

use serde_json::{json, Value};

use super::rules::{Condition, Field, Op, Rule};

#[derive(Debug, Clone, PartialEq)]
pub struct Parsed {
    pub rule: Rule,
    /// Tokens that looked like filters but could not be understood.
    pub warnings: Vec<String>,
}

fn tokenize(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    for c in input.chars() {
        match c {
            '"' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn fold(s: &str) -> String {
    crate::db::repo::games::sort_title(s).replace(' ', "-")
}

fn source_value(v: &str) -> Option<&'static str> {
    Some(match fold(v).as_str() {
        "steam" => "steam",
        "gog" | "galaxy" => "gog",
        "epic" | "epic-games" => "epic",
        "battlenet" | "battle.net" | "bnet" | "blizzard" => "battlenet",
        "ea" | "origin" => "ea",
        "ubisoft" | "uplay" => "ubisoft",
        "xbox" | "gamepass" => "xbox",
        "riot" => "riot",
        "local" | "exe" | "executable" | "executables" => "local",
        "emulateur" | "emulator" | "emulateurs" | "rom" => "emulator",
        _ => return None,
    })
}

fn status_value(v: &str) -> Option<&'static str> {
    Some(match fold(v).as_str() {
        "todo" | "a-faire" | "afaire" => "todo",
        "playing" | "en-cours" | "encours" => "playing",
        "done" | "termine" | "fini" | "finis" | "termines" => "done",
        "dropped" | "abandonne" | "abandonnes" => "dropped",
        _ => return None,
    })
}

fn bool_value(v: &str) -> Option<bool> {
    match fold(v).as_str() {
        "oui" | "yes" | "true" | "1" | "o" | "y" => Some(true),
        "non" | "no" | "false" | "0" | "n" => Some(false),
        _ => None,
    }
}

/// `>20`, `<=5`, `=3`, `10-20`, `12` → (op, value). `scale` converts units.
fn numeric(v: &str, scale: f64) -> Option<(Op, Value)> {
    let v = v.trim().replace(',', ".");
    let num = |s: &str| s.trim().parse::<f64>().ok().map(|n| n * scale);
    if let Some((a, b)) = v.split_once('-').filter(|(a, _)| !a.is_empty()) {
        return Some((Op::Between, json!([num(a)?, num(b)?])));
    }
    let (op, rest, adjust) = if let Some(r) = v.strip_prefix(">=") {
        (Op::Gt, r, -1e-9)
    } else if let Some(r) = v.strip_prefix("<=") {
        (Op::Lt, r, 1e-9)
    } else if let Some(r) = v.strip_prefix('>') {
        (Op::Gt, r, 0.0)
    } else if let Some(r) = v.strip_prefix('<') {
        (Op::Lt, r, 0.0)
    } else if let Some(r) = v.strip_prefix('=') {
        (Op::Eq, r, 0.0)
    } else {
        (Op::Eq, v.as_str(), 0.0)
    };
    Some((op, json!(num(rest)? + adjust)))
}

fn cond(field: Field, op: Op, value: Value) -> Rule {
    Rule::Cond(Condition { field, op, value })
}

/// One `key:value` token → rule, or `None` when not understood.
fn filter(key: &str, value: &str) -> Option<Rule> {
    if value.is_empty() {
        return None;
    }
    let text = |field| Some(cond(field, Op::In, json!([value])));
    match fold(key).as_str() {
        "source" | "src" | "plateforme" => {
            Some(cond(Field::Source, Op::In, json!([source_value(value)?])))
        }
        "tag" | "tags" => text(Field::Tag),
        "genre" | "genres" => text(Field::Genre),
        "mode" | "joueurs" | "players" => text(Field::Mode),
        "statut" | "status" => Some(cond(Field::Status, Op::In, json!([status_value(value)?]))),
        "heures" | "h" | "hours" | "temps" => {
            let (op, v) = numeric(value, 60.0)?;
            Some(cond(Field::PlaytimeMinutes, op, v))
        }
        "session" | "sessions" => {
            let (op, v) = numeric(value, 1.0)?;
            Some(cond(Field::AvgSessionMinutes, op, v))
        }
        "succes" | "achievements" | "succès" => {
            let (op, v) = numeric(value.trim_end_matches('%'), 1.0)?;
            Some(cond(Field::AchievementPercent, op, v))
        }
        "installe" | "installed" | "installé" => {
            Some(cond(Field::Installed, Op::Eq, json!(bool_value(value)?)))
        }
        "favori" | "fav" | "favorite" => {
            Some(cond(Field::Favorite, Op::Eq, json!(bool_value(value)?)))
        }
        // joue:<30 = played in the last 30 days; joue:>365 = not for a year.
        "joue" | "played" | "joué" => {
            let days = value.trim_end_matches(['j', 'd']);
            let (op, v) = numeric(days, 1.0)?;
            match op {
                Op::Lt => Some(cond(Field::LastPlayed, Op::NewerThanDays, v)),
                Op::Gt => Some(cond(Field::LastPlayed, Op::OlderThanDays, v)),
                _ => None,
            }
        }
        _ => None,
    }
}

pub fn parse(input: &str) -> Parsed {
    let mut rules = Vec::new();
    let mut words = Vec::new();
    let mut warnings = Vec::new();
    for token in tokenize(input) {
        let (negated, body) = match token.strip_prefix('-') {
            Some(rest) if rest.contains(':') => (true, rest),
            _ => (false, token.as_str()),
        };
        match body.split_once(':') {
            Some((key, value)) if !key.is_empty() => match filter(key, value) {
                Some(rule) => rules.push(if negated {
                    Rule::Not {
                        not: Box::new(rule),
                    }
                } else {
                    rule
                }),
                None => {
                    warnings.push(token.clone());
                    words.push(token.clone());
                }
            },
            _ => words.push(token.clone()),
        }
    }
    if !words.is_empty() {
        rules.push(cond(Field::Title, Op::Contains, json!(words.join(" "))));
    }
    Parsed {
        rule: Rule::All { all: rules },
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::rules::Facts;
    use chrono::{TimeZone, Utc};

    fn facts() -> Facts {
        Facts {
            title: "The Witcher 3: Wild Hunt".into(),
            tags: vec!["Souls like".into()],
            genres: vec!["RPG".into()],
            modes: vec!["Solo".into()],
            sources: vec!["gog".into()],
            installed: true,
            status: "done".into(),
            playtime_minutes: 3188.0,
            avg_session_minutes: Some(48.0),
            last_played: Some(Utc.with_ymd_and_hms(2026, 10, 10, 10, 0, 0).unwrap()),
            achievement_percent: Some(26.9),
            ..Default::default()
        }
    }

    fn hit(q: &str) -> bool {
        let p = parse(q);
        p.rule.validate().unwrap();
        p.rule.matches(
            &facts(),
            Utc.with_ymd_and_hms(2026, 11, 6, 0, 0, 0).unwrap(),
        )
    }

    #[test]
    fn documented_example() {
        assert!(hit("source:gog heures:>20 statut:done witcher"));
        assert!(!hit("source:steam witcher"));
    }

    #[test]
    fn french_values_and_aliases() {
        assert!(hit("statut:termine"));
        assert!(hit("statut:Terminé"));
        assert!(hit("src:galaxy installe:oui"));
        assert!(hit("genre:rpg joueurs:solo"));
        assert!(!hit("statut:en-cours"));
    }

    #[test]
    fn numbers_ranges_and_units() {
        assert!(hit("heures:50-60"));
        assert!(hit("heures:>=53"));
        assert!(!hit("heures:<20"));
        assert!(hit("session:<60"));
        assert!(hit("succes:<30%"));
        assert!(hit("joue:<30j"));
        assert!(!hit("joue:>365"));
    }

    #[test]
    fn quotes_negation_and_free_text() {
        assert!(hit(r#"tag:"souls like""#));
        assert!(!hit("-source:gog"));
        assert!(hit("-installe:non"));
        assert!(hit("wild hunt"));
        assert!(!hit("wild hunter"));
    }

    #[test]
    fn unknown_filters_become_text_with_a_warning() {
        let p = parse("platform:switch heures:beaucoup zelda");
        assert_eq!(p.warnings, vec!["platform:switch", "heures:beaucoup"]);
        assert_eq!(
            p.rule,
            Rule::All {
                all: vec![cond(
                    Field::Title,
                    Op::Contains,
                    json!("platform:switch heures:beaucoup zelda")
                )]
            }
        );
        assert_eq!(parse("").rule, Rule::All { all: vec![] });
    }
}
