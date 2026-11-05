//! SteamGridDB (https://www.steamgriddb.com/api/v2, Bearer key from the vault):
//! search a game, list covers (600x900 grids), heroes and logos.

use serde::Deserialize;

use crate::error::{AppError, AppResult};
use crate::http;

const API: &str = "https://www.steamgriddb.com/api/v2";

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SgdbGame {
    pub id: u32,
    pub name: String,
    #[serde(default)]
    pub release_date: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SgdbImage {
    pub id: u32,
    pub url: String,
    pub thumb: String,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default)]
    pub style: Option<String>,
}

#[derive(Deserialize)]
struct Envelope<T> {
    success: bool,
    #[serde(default = "Vec::new")]
    data: Vec<T>,
    #[serde(default)]
    errors: Vec<String>,
}

fn parse<T: for<'de> Deserialize<'de>>(body: &str) -> AppResult<Vec<T>> {
    let env: Envelope<T> = serde_json::from_str(body)?;
    if !env.success {
        return Err(AppError::Invalid(format!(
            "SteamGridDB: {}",
            env.errors.join(", ")
        )));
    }
    Ok(env.data)
}

pub fn parse_games(body: &str) -> AppResult<Vec<SgdbGame>> {
    parse(body)
}

pub fn parse_images(body: &str) -> AppResult<Vec<SgdbImage>> {
    parse(body)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ImageKind {
    Cover,
    Hero,
    Logo,
}

/// Only images served by SteamGridDB's CDN are downloaded.
pub fn is_sgdb_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    let host = rest.split('/').next().unwrap_or_default().to_lowercase();
    host == "steamgriddb.com" || host.ends_with(".steamgriddb.com")
}

fn get(path: &str, key: &str) -> AppResult<String> {
    let mut resp = http::agent()
        .get(&format!("{API}{path}"))
        .header("Authorization", &format!("Bearer {key}"))
        .call()
        .map_err(|e| match e {
            ureq::Error::StatusCode(401) | ureq::Error::StatusCode(403) => AppError::Network {
                status: Some(401),
                message: "SteamGridDB refused the API key".into(),
            },
            // 404 = no game / no image: an empty result.
            ureq::Error::StatusCode(404) => AppError::NotFound("SteamGridDB".into()),
            other => http::network_error(other, Some(key)),
        })?;
    resp.body_mut()
        .read_to_string()
        .map_err(|e| http::network_error(e, Some(key)))
}

fn empty_on_404<T>(r: AppResult<Vec<T>>) -> AppResult<Vec<T>> {
    match r {
        Err(AppError::NotFound(_)) => Ok(Vec::new()),
        other => other,
    }
}

pub fn search(key: &str, term: &str) -> AppResult<Vec<SgdbGame>> {
    let encoded: String = url_encode(term.trim());
    empty_on_404(get(&format!("/search/autocomplete/{encoded}"), key).and_then(|b| parse_games(&b)))
}

pub fn by_steam_appid(key: &str, appid: &str) -> AppResult<Option<SgdbGame>> {
    match get(&format!("/games/steam/{appid}"), key) {
        Ok(body) => {
            #[derive(Deserialize)]
            struct One {
                success: bool,
                data: Option<SgdbGame>,
            }
            let one: One = serde_json::from_str(&body)?;
            Ok(one.data.filter(|_| one.success))
        }
        Err(AppError::NotFound(_)) => Ok(None),
        Err(e) => Err(e),
    }
}

pub fn images(key: &str, game_id: u32, kind: ImageKind) -> AppResult<Vec<SgdbImage>> {
    let path = match kind {
        ImageKind::Cover => format!("/grids/game/{game_id}?dimensions=600x900&types=static"),
        ImageKind::Hero => format!("/heroes/game/{game_id}?types=static"),
        ImageKind::Logo => format!("/logos/game/{game_id}?types=static"),
    };
    empty_on_404(get(&path, key).and_then(|b| parse_images(&b)))
}

/// Percent-encodes a search term for the URL path.
pub fn url_encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_search_results_and_images() {
        let games = parse_games(
            r#"{"success":true,"data":[{"id":5254,"name":"The Witcher 3: Wild Hunt","release_date":1431993600,"types":["steam","gog"],"verified":true}]}"#,
        )
        .unwrap();
        assert_eq!(games[0].id, 5254);
        let images = parse_images(
            r#"{"success":true,"page":0,"total":1,"limit":50,"data":[{"id":1,"score":0,"style":"alternate","width":600,"height":900,"nsfw":false,"humor":false,"notes":null,"mime":"image/png","language":"en","url":"https://cdn2.steamgriddb.com/grid/a.png","thumb":"https://cdn2.steamgriddb.com/thumb/a.jpg","lock":false,"epilepsy":false,"upvotes":0,"downvotes":0,"author":{"name":"x","steam64":"1","avatar":"y"}}]}"#,
        )
        .unwrap();
        assert_eq!(images[0].width, Some(600));
        assert!(parse_games(r#"{"success":false,"errors":["Invalid key"]}"#).is_err());
    }

    #[test]
    fn only_steamgriddb_urls_are_accepted() {
        assert!(is_sgdb_url("https://cdn2.steamgriddb.com/grid/a.png"));
        assert!(!is_sgdb_url("http://cdn2.steamgriddb.com/grid/a.png"));
        assert!(!is_sgdb_url("https://steamgriddb.com.evil.example/a.png"));
        assert!(!is_sgdb_url("file:///C:/Windows/win.ini"));
    }

    #[test]
    fn encodes_search_terms() {
        assert_eq!(
            url_encode("The Witcher 3: Wild Hunt"),
            "The%20Witcher%203%3A%20Wild%20Hunt"
        );
        assert_eq!(url_encode("Pokémon"), "Pok%C3%A9mon");
    }
}
