//! Steam achievements: player progress, schema (French names) and global rarity.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::error::{AppError, AppResult};
use crate::http;

const API: &str = "https://api.steampowered.com";

/// At most one Steam Web API call per second (INTEGRATIONS.md).
fn throttle() {
    static LAST: Mutex<Option<Instant>> = Mutex::new(None);
    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(t) = *last {
        let wait = Duration::from_secs(1).saturating_sub(t.elapsed());
        if !wait.is_zero() {
            std::thread::sleep(wait);
        }
    }
    *last = Some(Instant::now());
}

/// One achievement merged from the three endpoints.
#[derive(Debug, Clone, PartialEq)]
pub struct SteamAchievement {
    pub api_name: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub icon_url: Option<String>,
    /// Unix seconds; `None` when locked.
    pub unlocked_at: Option<u64>,
    pub global_percent: Option<f64>,
    pub hidden: bool,
}

#[derive(Deserialize)]
struct PlayerEnvelope {
    playerstats: PlayerStats,
}

#[derive(Deserialize)]
struct PlayerStats {
    #[serde(default)]
    achievements: Vec<PlayerAchievement>,
    #[serde(default)]
    success: bool,
    error: Option<String>,
}

#[derive(Deserialize)]
struct PlayerAchievement {
    apiname: String,
    achieved: u8,
    #[serde(default)]
    unlocktime: u64,
}

#[derive(Deserialize)]
struct SchemaEnvelope {
    game: SchemaGame,
}

#[derive(Deserialize, Default)]
struct SchemaGame {
    #[serde(rename = "availableGameStats", default)]
    stats: Option<SchemaStats>,
}

#[derive(Deserialize, Default)]
struct SchemaStats {
    #[serde(default)]
    achievements: Vec<SchemaAchievement>,
}

#[derive(Deserialize)]
struct SchemaAchievement {
    name: String,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    description: Option<String>,
    icon: Option<String>,
    #[serde(default)]
    hidden: u8,
}

/// `apiname → (achieved, unlocktime)`. `Ok(None)` = the game has no achievements.
pub fn parse_player(body: &str) -> AppResult<Option<HashMap<String, Option<u64>>>> {
    let env: PlayerEnvelope = serde_json::from_str(body)?;
    if !env.playerstats.success {
        let msg = env.playerstats.error.unwrap_or_default();
        if msg.to_lowercase().contains("no stats") {
            return Ok(None);
        }
        return Err(AppError::Invalid(format!("Steam achievements: {msg}")));
    }
    Ok(Some(
        env.playerstats
            .achievements
            .into_iter()
            .map(|a| {
                (
                    a.apiname,
                    (a.achieved == 1).then_some(a.unlocktime).filter(|t| *t > 0),
                )
            })
            .collect(),
    ))
}

/// Name, French label, description and icon of one achievement.
#[derive(Debug, Clone, PartialEq)]
pub struct SchemaEntry {
    pub api_name: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub hidden: bool,
}

pub fn parse_schema(body: &str) -> AppResult<Vec<SchemaEntry>> {
    let env: SchemaEnvelope = serde_json::from_str(body)?;
    Ok(env
        .game
        .stats
        .unwrap_or_default()
        .achievements
        .into_iter()
        .map(|a| SchemaEntry {
            api_name: a.name,
            name: a.display_name,
            description: a.description.filter(|d| !d.is_empty()),
            icon: a.icon,
            hidden: a.hidden == 1,
        })
        .collect())
}

/// Percent can be a number or a string depending on the API version.
pub fn parse_global(body: &str) -> AppResult<HashMap<String, f64>> {
    let v: serde_json::Value = serde_json::from_str(body)?;
    let list = v
        .pointer("/achievementpercentages/achievements")
        .and_then(|a| a.as_array())
        .cloned()
        .unwrap_or_default();
    Ok(list
        .into_iter()
        .filter_map(|a| {
            let name = a.get("name")?.as_str()?.to_string();
            let pct = match a.get("percent")? {
                serde_json::Value::Number(n) => n.as_f64()?,
                serde_json::Value::String(s) => s.parse().ok()?,
                _ => return None,
            };
            Some((name, pct))
        })
        .collect())
}

fn get(url: &str, query: &[(&str, String)], key: Option<&str>) -> AppResult<Result<String, u16>> {
    throttle();
    let mut req = http::agent().get(url);
    for (k, v) in query {
        req = req.query(*k, v);
    }
    match req.call() {
        Ok(mut resp) => Ok(Ok(resp
            .body_mut()
            .read_to_string()
            .map_err(|e| http::network_error(e, key))?)),
        Err(ureq::Error::StatusCode(code)) => Ok(Err(code)),
        Err(e) => Err(http::network_error(e, key)),
    }
}

/// Fetches and merges everything. `Ok(None)` = no achievements for this game.
pub fn fetch(key: &str, steam_id: u64, appid: u64) -> AppResult<Option<Vec<SteamAchievement>>> {
    let player = get(
        &format!("{API}/ISteamUserStats/GetPlayerAchievements/v1/"),
        &[
            ("key", key.into()),
            ("steamid", steam_id.to_string()),
            ("appid", appid.to_string()),
            ("l", "french".into()),
        ],
        Some(key),
    )?;
    let unlocked = match player {
        Ok(body) => match parse_player(&body)? {
            Some(map) => map,
            None => return Ok(None),
        },
        // 400 is what Steam answers for games without stats.
        Err(400) => return Ok(None),
        Err(403) => {
            return Err(AppError::Network {
                status: Some(403),
                message: "Steam refused: the profile's game details are probably private".into(),
            })
        }
        Err(code) => {
            return Err(AppError::Network {
                status: Some(code),
                message: format!("HTTP {code}"),
            })
        }
    };

    let schema = match get(
        &format!("{API}/ISteamUserStats/GetSchemaForGame/v2/"),
        &[
            ("key", key.into()),
            ("appid", appid.to_string()),
            ("l", "french".into()),
        ],
        Some(key),
    )? {
        Ok(body) => parse_schema(&body)?,
        Err(_) => Vec::new(),
    };
    let global = match get(
        &format!("{API}/ISteamUserStats/GetGlobalAchievementPercentagesForApp/v2/"),
        &[("gameid", appid.to_string())],
        None,
    )? {
        Ok(body) => parse_global(&body).unwrap_or_default(),
        Err(_) => HashMap::new(),
    };

    Ok(Some(merge(unlocked, schema, global)))
}

pub fn merge(
    unlocked: HashMap<String, Option<u64>>,
    schema: Vec<SchemaEntry>,
    global: HashMap<String, f64>,
) -> Vec<SteamAchievement> {
    let mut list: Vec<SteamAchievement> = if schema.is_empty() {
        unlocked
            .iter()
            .map(|(name, at)| SteamAchievement {
                api_name: name.clone(),
                name: None,
                description: None,
                icon_url: None,
                unlocked_at: *at,
                global_percent: global.get(name).copied(),
                hidden: false,
            })
            .collect()
    } else {
        schema
            .into_iter()
            .map(
                |SchemaEntry {
                     api_name,
                     name,
                     description,
                     icon,
                     hidden,
                 }| SteamAchievement {
                    hidden,
                    unlocked_at: unlocked.get(&api_name).copied().flatten(),
                    global_percent: global.get(&api_name).copied(),
                    api_name,
                    name,
                    description,
                    icon_url: icon,
                },
            )
            .collect()
    };
    list.sort_by(|a, b| a.api_name.cmp(&b.api_name));
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_player_progress_and_no_stats() {
        let body = r#"{"playerstats":{"steamID":"1","gameName":"X","achievements":[
            {"apiname":"A","achieved":1,"unlocktime":1700000000},
            {"apiname":"B","achieved":0,"unlocktime":0}],"success":true}}"#;
        let map = parse_player(body).unwrap().unwrap();
        assert_eq!(map["A"], Some(1700000000));
        assert_eq!(map["B"], None);
        let none = r#"{"playerstats":{"error":"Requested app has no stats","success":false}}"#;
        assert_eq!(parse_player(none).unwrap(), None);
        let private = r#"{"playerstats":{"error":"Profile is not public","success":false}}"#;
        assert!(parse_player(private).is_err());
    }

    #[test]
    fn parses_schema_and_global_percentages() {
        let schema = r#"{"game":{"gameName":"X","availableGameStats":{"achievements":[
            {"name":"A","displayName":"Premier pas","description":"","icon":"https://x/a.jpg","hidden":0},
            {"name":"S","displayName":"Secret","icon":"https://x/s.jpg","hidden":1}]}}}"#;
        let s = parse_schema(schema).unwrap();
        assert_eq!(s[0].name.as_deref(), Some("Premier pas"));
        assert_eq!(s[0].description, None);
        assert!(!s[0].hidden && s[1].hidden);
        assert!(parse_schema(r#"{"game":{}}"#).unwrap().is_empty());

        let global = r#"{"achievementpercentages":{"achievements":[
            {"name":"A","percent":"45.5"},{"name":"B","percent":3.25},{"name":"C"}]}}"#;
        let g = parse_global(global).unwrap();
        assert_eq!(g["A"], 45.5);
        assert_eq!(g["B"], 3.25);
        assert!(!g.contains_key("C"));
    }

    #[test]
    fn merge_uses_the_schema_as_reference() {
        let unlocked = HashMap::from([("A".to_string(), Some(10)), ("B".to_string(), None)]);
        let schema = vec![
            SchemaEntry {
                api_name: "A".into(),
                name: Some("Un".into()),
                description: None,
                icon: None,
                hidden: false,
            },
            SchemaEntry {
                api_name: "B".into(),
                name: Some("Deux".into()),
                description: None,
                icon: None,
                hidden: true,
            },
        ];
        let list = merge(
            unlocked.clone(),
            schema,
            HashMap::from([("B".to_string(), 1.5)]),
        );
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].unlocked_at, Some(10));
        assert_eq!(list[1].global_percent, Some(1.5));
        assert!(list[1].hidden && !list[0].hidden);
        // Without a schema, the player list is used.
        assert_eq!(merge(unlocked, vec![], HashMap::new()).len(), 2);
    }
}
