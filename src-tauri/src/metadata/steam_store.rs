//! Steam Store media of a game (trailers, screenshots, short description),
//! shown on the game sheet. Public endpoints, no key:
//! `store.steampowered.com/api/appdetails` and `/api/storesearch` (used to
//! find the Steam page of games owned on another launcher).

use serde::{Deserialize, Serialize};

use crate::error::AppResult;
use crate::http;
use crate::library::merge::match_key;

#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
pub struct Trailer {
    pub name: String,
    pub thumbnail: String,
    /// HLS master playlist (Steam no longer serves plain MP4 trailers).
    pub hls: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
pub struct Screenshot {
    pub thumb: String,
    pub full: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, specta::Type)]
pub struct StoreMedia {
    pub description: Option<String>,
    pub trailers: Vec<Trailer>,
    pub screenshots: Vec<Screenshot>,
}

const MAX_TRAILERS: usize = 6;
const MAX_SCREENSHOTS: usize = 12;

/// The frontend loads these URLs directly (the CSP allows `*.steamstatic.com`
/// only): anything else returned by the store is dropped.
pub fn is_steam_cdn(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    host == "steamstatic.com" || host.ends_with(".steamstatic.com")
}

#[derive(Deserialize)]
struct Details {
    success: bool,
    data: Option<Data>,
}

#[derive(Deserialize)]
struct Data {
    #[serde(default)]
    short_description: Option<String>,
    #[serde(default)]
    movies: Vec<RawMovie>,
    #[serde(default)]
    screenshots: Vec<RawShot>,
}

#[derive(Deserialize)]
struct RawMovie {
    #[serde(default)]
    name: String,
    #[serde(default)]
    thumbnail: String,
    #[serde(default)]
    hls_h264: Option<String>,
    #[serde(default)]
    highlight: bool,
}

#[derive(Deserialize)]
struct RawShot {
    path_thumbnail: String,
    path_full: String,
}

/// Drops tags and decodes the few HTML entities found in store descriptions.
fn plain_text(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    let out = out
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
        .replace("&amp;", "&");
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `None` when the store has no page for this app (removed, region-locked…).
pub fn parse_details(body: &str, appid: &str) -> AppResult<Option<StoreMedia>> {
    let mut map: std::collections::HashMap<String, Details> = serde_json::from_str(body)?;
    let Some(Details {
        success: true,
        data: Some(data),
    }) = map.remove(appid)
    else {
        return Ok(None);
    };
    let mut movies = data.movies;
    // Highlighted trailers first, keeping the store order otherwise.
    movies.sort_by_key(|m| !m.highlight);
    let trailers = movies
        .into_iter()
        .filter_map(|m| {
            let hls = m.hls_h264?;
            (is_steam_cdn(&hls) && is_steam_cdn(&m.thumbnail)).then_some(Trailer {
                name: m.name,
                thumbnail: m.thumbnail,
                hls,
            })
        })
        .take(MAX_TRAILERS)
        .collect();
    let screenshots = data
        .screenshots
        .into_iter()
        .filter(|s| is_steam_cdn(&s.path_thumbnail) && is_steam_cdn(&s.path_full))
        .map(|s| Screenshot {
            thumb: s.path_thumbnail,
            full: s.path_full,
        })
        .take(MAX_SCREENSHOTS)
        .collect();
    let description = data
        .short_description
        .map(|d| plain_text(&d))
        .filter(|d| !d.is_empty());
    Ok(Some(StoreMedia {
        description,
        trailers,
        screenshots,
    }))
}

#[derive(Deserialize)]
struct Search {
    #[serde(default)]
    items: Vec<SearchItem>,
}

#[derive(Deserialize)]
struct SearchItem {
    #[serde(rename = "type", default)]
    kind: String,
    name: String,
    id: u64,
}

/// Steam app with exactly the same (normalised) title, if any: a near match
/// would show another game's trailer.
pub fn parse_search(body: &str, title: &str) -> AppResult<Option<String>> {
    let search: Search = serde_json::from_str(body)?;
    let key = match_key(title);
    if key.is_empty() {
        return Ok(None);
    }
    Ok(search
        .items
        .into_iter()
        .find(|i| i.kind == "app" && match_key(&i.name) == key)
        .map(|i| i.id.to_string()))
}

fn get(url: &str, query: &[(&str, &str)]) -> AppResult<String> {
    let mut req = http::agent().get(url);
    for (k, v) in query {
        req = req.query(*k, *v);
    }
    let mut resp = req.call().map_err(|e| http::network_error(e, None))?;
    resp.body_mut()
        .read_to_string()
        .map_err(|e| http::network_error(e, None))
}

pub fn fetch_details(appid: &str) -> AppResult<Option<StoreMedia>> {
    let body = get(
        "https://store.steampowered.com/api/appdetails",
        &[("appids", appid), ("l", "french")],
    )?;
    parse_details(&body, appid)
}

pub fn search_appid(title: &str) -> AppResult<Option<String>> {
    let body = get(
        "https://store.steampowered.com/api/storesearch/",
        &[("term", title), ("l", "french"), ("cc", "CH")],
    )?;
    parse_search(&body, title)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_only_steam_cdn_urls() {
        assert!(is_steam_cdn("https://shared.akamai.steamstatic.com/a.jpg"));
        assert!(is_steam_cdn(
            "https://video.akamai.steamstatic.com/x.m3u8?t=1"
        ));
        assert!(!is_steam_cdn("http://shared.akamai.steamstatic.com/a.jpg"));
        assert!(!is_steam_cdn("https://steamstatic.com.evil.example/a.jpg"));
        assert!(!is_steam_cdn("https://evil.example/steamstatic.com"));
    }

    #[test]
    fn parses_trailers_and_screenshots() {
        let body = r#"{"42":{"success":true,"data":{
          "short_description":"Un JDR &quot;sombre&quot; &amp; <b>ouvert</b>.",
          "movies":[
            {"id":1,"name":"Gameplay","thumbnail":"https://shared.akamai.steamstatic.com/1.jpg","hls_h264":"https://video.akamai.steamstatic.com/1.m3u8","highlight":false},
            {"id":2,"name":"Launch","thumbnail":"https://shared.akamai.steamstatic.com/2.jpg","hls_h264":"https://video.akamai.steamstatic.com/2.m3u8","highlight":true},
            {"id":3,"name":"No HLS","thumbnail":"https://shared.akamai.steamstatic.com/3.jpg"},
            {"id":4,"name":"Elsewhere","thumbnail":"https://x.example/4.jpg","hls_h264":"https://x.example/4.m3u8"}
          ],
          "screenshots":[
            {"id":0,"path_thumbnail":"https://shared.akamai.steamstatic.com/s.600x338.jpg","path_full":"https://shared.akamai.steamstatic.com/s.1920x1080.jpg"}
          ]}}}"#;
        let media = parse_details(body, "42").unwrap().unwrap();
        assert_eq!(
            media.description.as_deref(),
            Some("Un JDR \"sombre\" & ouvert.")
        );
        let names: Vec<_> = media.trailers.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["Launch", "Gameplay"]);
        assert_eq!(media.screenshots.len(), 1);
    }

    #[test]
    fn missing_store_page_is_none() {
        assert_eq!(
            parse_details(r#"{"7":{"success":false}}"#, "7").unwrap(),
            None
        );
    }

    #[test]
    fn search_requires_the_same_title() {
        let body = r#"{"total":3,"items":[
          {"type":"app","name":"Hades II","id":1145350},
          {"type":"app","name":"Hades","id":1145360}
        ]}"#;
        assert_eq!(
            parse_search(body, "HADES").unwrap().as_deref(),
            Some("1145360")
        );
        assert_eq!(parse_search(body, "Hades III").unwrap(), None);
    }
}
