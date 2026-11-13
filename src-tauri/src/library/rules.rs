//! Filter rules shared by smart collections and the search bar
//! (ARCHITECTURE.md § 6):
//!
//! ```json
//! { "all": [ { "field": "tag", "op": "in", "value": ["souls-like"] },
//!            { "any": [ ... ] } ] }
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Field {
    Title,
    Tag,
    Genre,
    Mode,
    Source,
    Installed,
    Status,
    Favorite,
    PlaytimeMinutes,
    AvgSessionMinutes,
    LastPlayed,
    AchievementPercent,
    HltbMainMinutes,
    AddedAt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Op {
    Eq,
    Neq,
    In,
    Lt,
    Gt,
    Between,
    OlderThanDays,
    NewerThanDays,
    Contains,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Condition {
    pub field: Field,
    pub op: Op,
    #[serde(default)]
    pub value: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Rule {
    All { all: Vec<Rule> },
    Any { any: Vec<Rule> },
    Not { not: Box<Rule> },
    Cond(Condition),
}

/// Everything a rule can look at for one game.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Facts {
    pub title: String,
    pub tags: Vec<String>,
    pub genres: Vec<String>,
    pub modes: Vec<String>,
    pub sources: Vec<String>,
    pub installed: bool,
    pub status: String,
    pub favorite: bool,
    pub playtime_minutes: f64,
    pub avg_session_minutes: Option<f64>,
    pub last_played: Option<DateTime<Utc>>,
    pub achievement_percent: Option<f64>,
    pub hltb_main_minutes: Option<f64>,
    pub added_at: Option<DateTime<Utc>>,
}

fn fold(s: &str) -> String {
    crate::db::repo::games::sort_title(s)
}

fn strings(v: &Value) -> Vec<String> {
    match v {
        Value::String(s) => vec![s.clone()],
        Value::Array(items) => items
            .iter()
            .filter_map(|i| i.as_str().map(str::to_string))
            .collect(),
        other => vec![other.to_string()],
    }
}

fn number(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().replace(',', ".").parse().ok(),
        _ => None,
    }
}

fn pair(v: &Value) -> Option<(f64, f64)> {
    let a = v.as_array()?;
    Some((number(a.first()?)?, number(a.get(1)?)?))
}

fn date(v: &Value) -> Option<DateTime<Utc>> {
    v.as_str()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.with_timezone(&Utc))
        .or_else(|| {
            v.as_str()
                .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
                .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc())
        })
}

fn set_matches(values: &[String], op: Op, wanted: &Value) -> Option<bool> {
    let wanted: Vec<String> = strings(wanted).iter().map(|w| fold(w)).collect();
    let have: Vec<String> = values.iter().map(|v| fold(v)).collect();
    let any = wanted.iter().any(|w| have.contains(w));
    match op {
        Op::Eq | Op::In => Some(any),
        Op::Neq => Some(!any),
        Op::Contains => Some(
            wanted
                .iter()
                .any(|w| have.iter().any(|h| h.contains(w.as_str()))),
        ),
        _ => None,
    }
}

fn number_matches(value: Option<f64>, op: Op, wanted: &Value) -> Option<bool> {
    if op == Op::Between {
        let (lo, hi) = pair(wanted)?;
        return Some(value.is_some_and(|v| v >= lo && v <= hi));
    }
    let w = number(wanted)?;
    Some(match (op, value) {
        (Op::Neq, None) => true,
        (_, None) => false,
        (Op::Eq, Some(v)) => (v - w).abs() < f64::EPSILON,
        (Op::Neq, Some(v)) => (v - w).abs() >= f64::EPSILON,
        (Op::Lt, Some(v)) => v < w,
        (Op::Gt, Some(v)) => v > w,
        _ => return None,
    })
}

/// `never_counts_as_old`: a game never played is "not played for a year".
fn date_matches(
    value: Option<DateTime<Utc>>,
    op: Op,
    wanted: &Value,
    now: DateTime<Utc>,
    never_counts_as_old: bool,
) -> Option<bool> {
    match op {
        Op::OlderThanDays | Op::NewerThanDays => {
            let days = number(wanted)?;
            let limit = now - chrono::Duration::seconds((days * 86_400.0) as i64);
            Some(match value {
                None => op == Op::OlderThanDays && never_counts_as_old,
                Some(d) if op == Op::OlderThanDays => d < limit,
                Some(d) => d >= limit,
            })
        }
        Op::Lt => Some(value.is_some_and(|d| d < date(wanted).unwrap_or(now))),
        Op::Gt => Some(value.is_some_and(|d| d > date(wanted).unwrap_or(now))),
        _ => None,
    }
}

fn eval_condition(c: &Condition, f: &Facts, now: DateTime<Utc>) -> Option<bool> {
    match c.field {
        Field::Title => {
            let title = fold(&f.title);
            let wanted = strings(&c.value);
            match c.op {
                Op::Contains => Some(wanted.iter().any(|w| title.contains(&fold(w)))),
                Op::Eq | Op::In => Some(wanted.iter().any(|w| title == fold(w))),
                Op::Neq => Some(wanted.iter().all(|w| title != fold(w))),
                _ => None,
            }
        }
        Field::Tag => set_matches(&f.tags, c.op, &c.value),
        // Player modes count as genres too: people look for "multijoueur" or
        // "coop" as a genre, and the two lists share no name.
        Field::Genre => {
            let both: Vec<String> = f.genres.iter().chain(&f.modes).cloned().collect();
            set_matches(&both, c.op, &c.value)
        }
        Field::Mode => set_matches(&f.modes, c.op, &c.value),
        Field::Source => set_matches(&f.sources, c.op, &c.value),
        Field::Status => set_matches(std::slice::from_ref(&f.status), c.op, &c.value),
        Field::Installed | Field::Favorite => {
            let actual = if c.field == Field::Installed {
                f.installed
            } else {
                f.favorite
            };
            let wanted = c.value.as_bool().unwrap_or(true);
            match c.op {
                Op::Eq => Some(actual == wanted),
                Op::Neq => Some(actual != wanted),
                _ => None,
            }
        }
        Field::PlaytimeMinutes => number_matches(Some(f.playtime_minutes), c.op, &c.value),
        Field::AvgSessionMinutes => number_matches(f.avg_session_minutes, c.op, &c.value),
        Field::AchievementPercent => number_matches(f.achievement_percent, c.op, &c.value),
        Field::HltbMainMinutes => number_matches(f.hltb_main_minutes, c.op, &c.value),
        Field::LastPlayed => date_matches(f.last_played, c.op, &c.value, now, true),
        Field::AddedAt => date_matches(f.added_at, c.op, &c.value, now, false),
    }
}

impl Rule {
    pub fn matches(&self, facts: &Facts, now: DateTime<Utc>) -> bool {
        match self {
            Rule::All { all } => all.iter().all(|r| r.matches(facts, now)),
            Rule::Any { any } => any.is_empty() || any.iter().any(|r| r.matches(facts, now)),
            Rule::Not { not } => !not.matches(facts, now),
            // Validated beforehand; an invalid combination simply does not match.
            Rule::Cond(c) => eval_condition(c, facts, now).unwrap_or(false),
        }
    }

    /// Checks every condition (field/op/value combination) before saving.
    pub fn validate(&self) -> AppResult<()> {
        match self {
            Rule::All { all } => all.iter().try_for_each(Rule::validate),
            Rule::Any { any } => any.iter().try_for_each(Rule::validate),
            Rule::Not { not } => not.validate(),
            Rule::Cond(c) => {
                let probe = Facts::default();
                eval_condition(c, &probe, Utc::now())
                    .map(|_| ())
                    .ok_or_else(|| {
                        AppError::Invalid(format!(
                            "invalid rule: {:?} {:?} {}",
                            c.field, c.op, c.value
                        ))
                    })
            }
        }
    }

    pub fn parse(json: &str) -> AppResult<Rule> {
        let rule: Rule = serde_json::from_str(json)?;
        rule.validate()?;
        Ok(rule)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use serde_json::json;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 11, 6, 12, 0, 0).unwrap()
    }

    fn souls() -> Facts {
        Facts {
            title: "DARK SOULS III".into(),
            tags: vec!["Souls-like".into(), "Difficile".into()],
            genres: vec!["Action".into(), "RPG".into()],
            modes: vec!["Solo".into()],
            sources: vec!["steam".into()],
            installed: true,
            status: "playing".into(),
            favorite: false,
            playtime_minutes: 14_418.0,
            avg_session_minutes: Some(95.0),
            last_played: Some(Utc.with_ymd_and_hms(2026, 10, 20, 11, 0, 0).unwrap()),
            achievement_percent: Some(79.0),
            hltb_main_minutes: None,
            added_at: Some(Utc.with_ymd_and_hms(2026, 11, 4, 23, 9, 0).unwrap()),
        }
    }

    fn rule(v: serde_json::Value) -> Rule {
        let r: Rule = serde_json::from_value(v).unwrap();
        r.validate().unwrap();
        r
    }

    #[test]
    fn parses_the_documented_example() {
        let r = Rule::parse(
            r#"{ "all": [
              { "field": "tag", "op": "in", "value": ["souls-like"] },
              { "field": "avg_session_minutes", "op": "lt", "value": 60 },
              { "field": "last_played", "op": "older_than_days", "value": 365 },
              { "field": "source", "op": "eq", "value": "gog" },
              { "field": "status", "op": "in", "value": ["todo", "dropped"] }
            ]}"#,
        )
        .unwrap();
        assert!(!r.matches(&souls(), now()));
    }

    #[test]
    fn sets_are_case_and_accent_insensitive() {
        let f = souls();
        assert!(
            rule(json!({"field": "tag", "op": "in", "value": ["souls-like"]})).matches(&f, now())
        );
        assert!(rule(json!({"field": "genre", "op": "eq", "value": "rpg"})).matches(&f, now()));
        assert!(rule(json!({"field": "source", "op": "neq", "value": "gog"})).matches(&f, now()));
        assert!(
            rule(json!({"field": "tag", "op": "contains", "value": "souls"})).matches(&f, now())
        );
        assert!(
            !rule(json!({"field": "status", "op": "in", "value": ["todo", "dropped"]}))
                .matches(&f, now())
        );
    }

    #[test]
    fn numbers_and_ranges() {
        let f = souls();
        assert!(
            rule(json!({"field": "playtime_minutes", "op": "gt", "value": 1200}))
                .matches(&f, now())
        );
        assert!(
            rule(json!({"field": "avg_session_minutes", "op": "between", "value": [60, 120]}))
                .matches(&f, now())
        );
        assert!(
            rule(json!({"field": "achievement_percent", "op": "lt", "value": "80"}))
                .matches(&f, now())
        );
        // Unknown value: only `neq` matches.
        assert!(
            !rule(json!({"field": "hltb_main_minutes", "op": "lt", "value": 600}))
                .matches(&f, now())
        );
        assert!(
            rule(json!({"field": "hltb_main_minutes", "op": "neq", "value": 600}))
                .matches(&f, now())
        );
    }

    #[test]
    fn dates_relative_to_now() {
        let f = souls();
        assert!(
            rule(json!({"field": "last_played", "op": "newer_than_days", "value": 30}))
                .matches(&f, now())
        );
        assert!(
            !rule(json!({"field": "last_played", "op": "older_than_days", "value": 30}))
                .matches(&f, now())
        );
        let never = Facts {
            last_played: None,
            ..souls()
        };
        // "Not launched for a year" includes games never launched.
        assert!(
            rule(json!({"field": "last_played", "op": "older_than_days", "value": 365}))
                .matches(&never, now())
        );
        assert!(
            rule(json!({"field": "added_at", "op": "gt", "value": "2026-11-01"}))
                .matches(&f, now())
        );
    }

    #[test]
    fn nested_groups_and_negation() {
        let r = rule(json!({ "any": [
            { "all": [ {"field": "installed", "op": "eq", "value": true}, {"field": "favorite", "op": "eq", "value": true} ] },
            { "not": {"field": "title", "op": "contains", "value": "souls"} }
        ]}));
        assert!(!r.matches(&souls(), now()));
        let fav = Facts {
            favorite: true,
            ..souls()
        };
        assert!(r.matches(&fav, now()));
        assert!(rule(json!({"all": []})).matches(&souls(), now()));
    }

    #[test]
    fn rejects_invalid_combinations() {
        for bad in [
            json!({"field": "installed", "op": "gt", "value": 3}),
            json!({"field": "playtime_minutes", "op": "contains", "value": "x"}),
            json!({"field": "playtime_minutes", "op": "between", "value": [1]}),
            json!({"field": "last_played", "op": "older_than_days", "value": "soon"}),
        ] {
            let r: Rule = serde_json::from_value(bad.clone()).unwrap();
            assert!(r.validate().is_err(), "{bad}");
        }
        assert!(Rule::parse(r#"{"field": "nope", "op": "eq"}"#).is_err());
    }

    #[test]
    fn genre_also_matches_player_modes() {
        let f = Facts {
            genres: vec!["Action".into()],
            modes: vec!["Multijoueur".into(), "Coopération".into()],
            ..Default::default()
        };
        let g = |op: &str, v: &str| rule(json!({"field": "genre", "op": op, "value": [v]}));
        assert!(g("in", "multijoueur").matches(&f, now()));
        assert!(g("in", "Cooperation").matches(&f, now()));
        assert!(g("in", "Action").matches(&f, now()));
        assert!(!g("neq", "Multijoueur").matches(&f, now()));
        assert!(!g("in", "RPG").matches(&f, now()));
        // A mode condition still only looks at modes.
        assert!(
            !rule(json!({"field": "mode", "op": "in", "value": ["Action"]})).matches(&f, now())
        );
    }
}
