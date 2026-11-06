//! IGDB (Twitch) media for games without a usable Steam page: screenshots,
//! artworks, YouTube videos and summary.
//!
//! Auth: Twitch client credentials (`POST id.twitch.tv/oauth2/token`), the
//! client ID and secret come from the OS vault; the app token is kept in
//! memory only. Queries: `POST api.igdb.com/v4/games` (Apicalypse body).

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::error::{AppError, AppResult};
use crate::http;
use crate::library::merge::match_key;

use super::steam_store::{Screenshot, Trailer};

const TOKEN_URL: &str = "https://id.twitch.tv/oauth2/token";
const GAMES_URL: &str = "https://api.igdb.com/v4/games";
const FIELDS: &str = "fields name,summary,screenshots.image_id,artworks.image_id,\
videos.video_id,videos.name;";
const MAX_IMAGES: usize = 12;
const MAX_VIDEOS: usize = 6;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct IgdbMedia {
    pub id: i64,
    pub summary: Option<String>,
    pub trailers: Vec<Trailer>,
    pub screenshots: Vec<Screenshot>,
}

#[derive(Deserialize)]
struct RawGame {
    id: i64,
    #[serde(default)]
    name: String,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    screenshots: Vec<RawImage>,
    #[serde(default)]
    artworks: Vec<RawImage>,
    #[serde(default)]
    videos: Vec<RawVideo>,
}

#[derive(Deserialize)]
struct RawImage {
    image_id: String,
}

#[derive(Deserialize)]
struct RawVideo {
    video_id: String,
    #[serde(default)]
    name: Option<String>,
}

fn image(size: &str, id: &str) -> String {
    format!("https://images.igdb.com/igdb/image/upload/t_{size}/{id}.jpg")
}

/// YouTube ids are 11 chars of `[A-Za-z0-9_-]`; anything else is dropped
/// (it ends up in an embed URL).
pub fn is_youtube_id(id: &str) -> bool {
    id.len() == 11
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn is_image_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric())
}

fn to_media(g: RawGame) -> IgdbMedia {
    let trailers = g
        .videos
        .into_iter()
        .filter(|v| is_youtube_id(&v.video_id))
        .take(MAX_VIDEOS)
        .map(|v| Trailer {
            name: v
                .name
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| g.name.clone()),
            thumbnail: format!("https://i.ytimg.com/vi/{}/hqdefault.jpg", v.video_id),
            hls: None,
            youtube: Some(v.video_id),
        })
        .collect();
    // Screenshots first, then artworks (key art) to fill up.
    let screenshots = g
        .screenshots
        .iter()
        .chain(g.artworks.iter())
        .filter(|i| is_image_id(&i.image_id))
        .take(MAX_IMAGES)
        .map(|i| Screenshot {
            thumb: image("screenshot_med", &i.image_id),
            full: image("1080p", &i.image_id),
        })
        .collect();
    IgdbMedia {
        id: g.id,
        summary: g
            .summary
            .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
            .filter(|s| !s.is_empty()),
        trailers,
        screenshots,
    }
}

/// Game with exactly the same normalised title among search results.
pub fn parse_search(body: &str, title: &str) -> AppResult<Option<IgdbMedia>> {
    let games: Vec<RawGame> = serde_json::from_str(body)?;
    let key = match_key(title);
    if key.is_empty() {
        return Ok(None);
    }
    Ok(games
        .into_iter()
        .find(|g| match_key(&g.name) == key)
        .map(to_media))
}

pub fn parse_by_id(body: &str) -> AppResult<Option<IgdbMedia>> {
    let games: Vec<RawGame> = serde_json::from_str(body)?;
    Ok(games.into_iter().next().map(to_media))
}

/// Apicalypse search string: quotes and backslashes would end it early.
pub fn search_query(title: &str) -> String {
    let clean: String = title.chars().filter(|c| *c != '"' && *c != '\\').collect();
    format!("search \"{}\"; {FIELDS} limit 10;", clean.trim())
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: u64,
}

/// `(client id, token, valid until)`.
static TOKEN: Mutex<Option<(String, String, Instant)>> = Mutex::new(None);

fn token(client_id: &str, secret: &str) -> AppResult<String> {
    {
        let guard = TOKEN.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((id, token, until)) = guard.as_ref() {
            if id == client_id && Instant::now() < *until {
                return Ok(token.clone());
            }
        }
    }
    let mut resp = http::agent()
        .post(TOKEN_URL)
        .query("client_id", client_id)
        .query("client_secret", secret)
        .query("grant_type", "client_credentials")
        .send_empty()
        .map_err(|e| http::network_error(e, Some(secret)))?;
    let body = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| http::network_error(e, Some(secret)))?;
    let parsed: TokenResponse = serde_json::from_str(&body)?;
    // Renewed a minute early.
    let until = Instant::now() + Duration::from_secs(parsed.expires_in.saturating_sub(60));
    *TOKEN.lock().unwrap_or_else(|e| e.into_inner()) =
        Some((client_id.to_string(), parsed.access_token.clone(), until));
    Ok(parsed.access_token)
}

fn forget_token() {
    *TOKEN.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

fn query(client_id: &str, secret: &str, body: &str) -> AppResult<String> {
    for attempt in 0..2 {
        let token = token(client_id, secret)?;
        let result = http::agent()
            .post(GAMES_URL)
            .header("Client-ID", client_id)
            .header("Authorization", &format!("Bearer {token}"))
            .header("Accept", "application/json")
            .send(body);
        match result {
            Ok(mut resp) => {
                return resp
                    .body_mut()
                    .read_to_string()
                    .map_err(|e| http::network_error(e, Some(&token)));
            }
            // Expired or revoked token: get a new one once.
            Err(ureq::Error::StatusCode(401)) if attempt == 0 => forget_token(),
            Err(e) => return Err(http::network_error(e, Some(&token))),
        }
    }
    Err(AppError::Invalid("IGDB refused the token".into()))
}

/// Media of the IGDB game `known_id`, else of the game with the same title.
pub fn fetch(
    client_id: &str,
    secret: &str,
    known_id: Option<i64>,
    title: &str,
) -> AppResult<Option<IgdbMedia>> {
    if let Some(id) = known_id {
        let body = query(client_id, secret, &format!("{FIELDS} where id = {id};"))?;
        if let Some(found) = parse_by_id(&body)? {
            return Ok(Some(found));
        }
    }
    let body = query(client_id, secret, &search_query(title))?;
    parse_search(&body, title)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BODY: &str = r#"[
      {"id":1,"name":"Hades II","summary":"Sequel."},
      {"id":2,"name":"Hades","summary":"  A rogue-like\n dungeon crawler. ",
       "screenshots":[{"id":9,"image_id":"sc6lbb"}],
       "artworks":[{"id":8,"image_id":"ar5x"}],
       "videos":[{"id":7,"video_id":"91t0ha9x0AE","name":"Launch Trailer"},
                 {"id":6,"video_id":"bad id\"><script>"}]}
    ]"#;

    #[test]
    fn picks_the_same_title_and_builds_urls() {
        let media = parse_search(BODY, "HADES").unwrap().unwrap();
        assert_eq!(media.id, 2);
        assert_eq!(
            media.summary.as_deref(),
            Some("A rogue-like dungeon crawler.")
        );
        assert_eq!(media.trailers.len(), 1);
        assert_eq!(media.trailers[0].youtube.as_deref(), Some("91t0ha9x0AE"));
        assert_eq!(media.trailers[0].name, "Launch Trailer");
        assert_eq!(
            media.screenshots[0].full,
            "https://images.igdb.com/igdb/image/upload/t_1080p/sc6lbb.jpg"
        );
        assert_eq!(media.screenshots.len(), 2);
        assert_eq!(parse_search(BODY, "Hades III").unwrap(), None);
    }

    #[test]
    fn search_query_cannot_be_broken_by_the_title() {
        assert_eq!(
            search_query(r#"Say "Hi"\ "#),
            format!("search \"Say Hi\"; {FIELDS} limit 10;")
        );
    }

    #[test]
    fn youtube_ids_are_checked() {
        assert!(is_youtube_id("91t0ha9x0AE"));
        assert!(is_youtube_id("a-b_c-d_e-f"));
        assert!(!is_youtube_id("short"));
        assert!(!is_youtube_id("91t0ha9x0A&"));
    }
}
