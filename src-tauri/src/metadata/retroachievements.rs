//! RetroAchievements Web API (https://retroachievements.org/API/, username
//! + Web API key from the vault).

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

use crate::db::repo::achievements::AchievementRow;
use crate::error::{AppError, AppResult};
use crate::http;

const API: &str = "https://retroachievements.org/API";
/// Game lists per console change rarely: cached for a week.
const LIST_CACHE_DAYS: u64 = 7;

#[derive(Debug, Clone, PartialEq, Deserialize, serde::Serialize)]
pub struct RaGame {
    #[serde(rename = "ID")]
    pub id: u32,
    #[serde(rename = "Title")]
    pub title: String,
    #[serde(rename = "Hashes", default)]
    pub hashes: Vec<String>,
}

pub fn parse_game_list(body: &str) -> AppResult<Vec<RaGame>> {
    Ok(serde_json::from_str(body)?)
}

/// RA game id whose hash list contains `hash`.
pub fn find_by_hash(games: &[RaGame], hash: &str) -> Option<u32> {
    games
        .iter()
        .find(|g| g.hashes.iter().any(|h| h.eq_ignore_ascii_case(hash)))
        .map(|g| g.id)
}

fn get(path: &str, query: &[(&str, String)], key: &str) -> AppResult<String> {
    let mut req = http::agent().get(&format!("{API}/{path}")).query("y", key);
    for (k, v) in query {
        req = req.query(*k, v);
    }
    let mut resp = req.call().map_err(|e| match e {
        ureq::Error::StatusCode(401) | ureq::Error::StatusCode(403) => AppError::Network {
            status: Some(401),
            message: "RetroAchievements refused the API key".into(),
        },
        other => http::network_error(other, Some(key)),
    })?;
    resp.body_mut()
        .read_to_string()
        .map_err(|e| http::network_error(e, Some(key)))
}

/// Games of a console with their hashes, from a 7-day disk cache.
pub fn game_list(cache_dir: &Path, key: &str, console_id: u32) -> AppResult<Vec<RaGame>> {
    let file = cache_dir.join(format!("console_{console_id}.json"));
    let fresh = std::fs::metadata(&file)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok())
        .is_some_and(|age| age.as_secs() < LIST_CACHE_DAYS * 86_400);
    if fresh {
        if let Ok(list) = std::fs::read(&file)
            .map_err(AppError::from)
            .and_then(|b| serde_json::from_slice::<Vec<RaGame>>(&b).map_err(AppError::from))
        {
            return Ok(list);
        }
    }
    let body = get(
        "API_GetGameList.php",
        &[
            ("i", console_id.to_string()),
            ("h", "1".into()),
            ("f", "1".into()),
        ],
        key,
    )?;
    let list = parse_game_list(&body)?;
    std::fs::create_dir_all(cache_dir)?;
    std::fs::write(&file, serde_json::to_vec(&list)?)?;
    Ok(list)
}

#[derive(Deserialize)]
struct Progress {
    #[serde(rename = "NumDistinctPlayers", default)]
    players: Option<serde_json::Value>,
    #[serde(rename = "Achievements", default)]
    achievements: HashMap<String, RaAchievement>,
}

#[derive(Deserialize)]
struct RaAchievement {
    #[serde(rename = "ID")]
    id: u32,
    #[serde(rename = "Title")]
    title: String,
    #[serde(rename = "Description", default)]
    description: Option<String>,
    #[serde(rename = "NumAwarded", default)]
    awarded: Option<serde_json::Value>,
    #[serde(rename = "DateEarned", default)]
    earned: Option<String>,
    #[serde(rename = "DateEarnedHardcore", default)]
    earned_hardcore: Option<String>,
    #[serde(rename = "BadgeName", default)]
    badge: Option<String>,
}

fn num(v: &Option<serde_json::Value>) -> Option<f64> {
    match v.as_ref()? {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

/// RA dates are `YYYY-MM-DD HH:MM:SS` (UTC).
fn ra_date(s: &str) -> Option<String> {
    chrono::NaiveDateTime::parse_from_str(s.trim(), "%Y-%m-%d %H:%M:%S")
        .ok()
        .map(|d| {
            d.and_utc()
                .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
        })
}

pub fn parse_progress(body: &str) -> AppResult<Vec<AchievementRow>> {
    let p: Progress = serde_json::from_str(body)?;
    let players = num(&p.players).filter(|n| *n > 0.0);
    let mut rows: Vec<AchievementRow> = p
        .achievements
        .into_values()
        .map(|a| AchievementRow {
            api_name: a.id.to_string(),
            name: Some(a.title),
            description: a.description.filter(|d| !d.is_empty()),
            icon_url: a
                .badge
                .map(|b| format!("https://media.retroachievements.org/Badge/{b}.png")),
            unlocked_at: a.earned_hardcore.or(a.earned).as_deref().and_then(ra_date),
            global_percent: match (num(&a.awarded), players) {
                (Some(n), Some(total)) => Some((n / total * 1000.0).round() / 10.0),
                _ => None,
            },
        })
        .collect();
    rows.sort_by(|a, b| a.api_name.cmp(&b.api_name));
    Ok(rows)
}

pub fn progress(key: &str, user: &str, game_id: u32) -> AppResult<Vec<AchievementRow>> {
    let body = get(
        "API_GetGameInfoAndUserProgress.php",
        &[("g", game_id.to_string()), ("u", user.to_string())],
        key,
    )?;
    parse_progress(&body)
}

/// Consoles offered in the emulator settings (RA ids).
pub const CONSOLES: &[(u32, &str)] = &[
    (7, "NES / Famicom"),
    (3, "Super Nintendo"),
    (2, "Nintendo 64"),
    (4, "Game Boy"),
    (6, "Game Boy Color"),
    (5, "Game Boy Advance"),
    (18, "Nintendo DS"),
    (16, "GameCube"),
    (1, "Mega Drive / Genesis"),
    (11, "Master System"),
    (15, "Game Gear"),
    (12, "PlayStation"),
    (21, "PlayStation 2"),
    (41, "PSP"),
    (25, "Atari 2600"),
    (27, "Arcade"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_game_list_and_matches_hashes() {
        let list = parse_game_list(
            r#"[{"Title":"Super Metroid","ID":1234,"ConsoleID":3,"ImageIcon":"/x.png","NumAchievements":50,
                "Hashes":["A0B1C2D3E4F5A6B7C8D9E0F1A2B3C4D5"]}]"#,
        )
        .unwrap();
        assert_eq!(
            find_by_hash(&list, "a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5"),
            Some(1234)
        );
        assert_eq!(find_by_hash(&list, "ffff"), None);
    }

    #[test]
    fn parses_progress_with_rarity_and_dates() {
        let body = r#"{"ID":1234,"Title":"Super Metroid","NumDistinctPlayers":"200","Achievements":{
            "1":{"ID":1,"Title":"Morph Ball","Description":"Get it","NumAwarded":150,"BadgeName":"001",
                 "DateEarned":"2024-01-02 10:00:00","DateEarnedHardcore":"2024-01-02 09:00:00"},
            "2":{"ID":2,"Title":"Ridley","Description":"","NumAwarded":"20"}}}"#;
        let rows = parse_progress(body).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].unlocked_at.as_deref(), Some("2024-01-02T09:00:00Z"));
        assert_eq!(rows[0].global_percent, Some(75.0));
        assert_eq!(
            rows[0].icon_url.as_deref(),
            Some("https://media.retroachievements.org/Badge/001.png")
        );
        assert_eq!(rows[1].unlocked_at, None);
        assert_eq!(rows[1].description, None);
        assert_eq!(rows[1].global_percent, Some(10.0));
    }
}
