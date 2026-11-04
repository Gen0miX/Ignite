//! Steam Web API (key typed by the user, stored in the OS vault).

use serde::Deserialize;

use crate::error::{AppError, AppResult};
use crate::http;

const OWNED_GAMES_URL: &str = "https://api.steampowered.com/IPlayerService/GetOwnedGames/v1/";

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct OwnedGame {
    pub appid: u64,
    #[serde(default)]
    pub name: Option<String>,
    /// Minutes.
    #[serde(default)]
    pub playtime_forever: u64,
    /// Unix seconds, 0 when never played.
    #[serde(default)]
    pub rtime_last_played: u64,
}

#[derive(Deserialize)]
struct Envelope {
    response: OwnedGamesResponse,
}

#[derive(Deserialize)]
struct OwnedGamesResponse {
    game_count: Option<u64>,
    #[serde(default)]
    games: Vec<OwnedGame>,
}

/// Parses a `GetOwnedGames` body. An empty `response` (no `game_count`) means
/// the profile's game details are private.
pub fn parse_owned_games(body: &str) -> AppResult<Vec<OwnedGame>> {
    let env: Envelope = serde_json::from_str(body)?;
    if env.response.game_count.is_none() {
        return Err(AppError::Invalid(
            "Steam returned no games: the profile's game details are probably private".into(),
        ));
    }
    Ok(env.response.games)
}

pub fn get_owned_games(key: &str, steam_id: u64) -> AppResult<Vec<OwnedGame>> {
    let body = http::agent()
        .get(OWNED_GAMES_URL)
        .query("key", key)
        .query("steamid", steam_id.to_string())
        .query("include_appinfo", "1")
        .query("include_played_free_games", "1")
        .query("format", "json")
        .call()
        .map_err(|e| http::network_error(e, Some(key)))?
        .body_mut()
        .read_to_string()
        .map_err(|e| http::network_error(e, Some(key)))?;
    parse_owned_games(&body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_owned_games() {
        let body = r#"{"response":{"game_count":2,"games":[
            {"appid":374320,"name":"DARK SOULS™ III","playtime_forever":14400,"rtime_last_played":1700000000,"img_icon_url":"x"},
            {"appid":10,"playtime_forever":0}
        ]}}"#;
        let games = parse_owned_games(body).unwrap();
        assert_eq!(games.len(), 2);
        assert_eq!(games[0].name.as_deref(), Some("DARK SOULS™ III"));
        assert_eq!(games[0].playtime_forever, 14400);
        assert_eq!(games[1].name, None);
        assert_eq!(games[1].rtime_last_played, 0);
    }

    #[test]
    fn private_profile_is_an_error() {
        assert!(parse_owned_games(r#"{"response":{}}"#).is_err());
        assert_eq!(
            parse_owned_games(r#"{"response":{"game_count":0}}"#).unwrap(),
            vec![]
        );
    }
}
