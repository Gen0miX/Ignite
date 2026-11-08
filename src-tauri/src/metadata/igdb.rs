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
const TIME_TO_BEAT_URL: &str = "https://api.igdb.com/v4/game_time_to_beats";
const FIELDS: &str =
    "fields name,summary,total_rating_count,screenshots.image_id,artworks.image_id,\
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
    total_rating_count: Option<u32>,
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

/// Game with exactly the same normalised title among search results. Several
/// games can share a title ("Hades"): the one with the most media, then the
/// most ratings, wins (the first one on a tie).
pub fn parse_search(body: &str, title: &str) -> AppResult<Option<IgdbMedia>> {
    let games: Vec<RawGame> = serde_json::from_str(body)?;
    let key = match_key(title);
    if key.is_empty() {
        return Ok(None);
    }
    let score = |g: &RawGame| {
        (
            g.videos.len() + g.screenshots.len() + g.artworks.len(),
            g.total_rating_count.unwrap_or(0),
        )
    };
    let mut best: Option<RawGame> = None;
    for g in games.into_iter().filter(|g| match_key(&g.name) == key) {
        if best.as_ref().is_none_or(|b| score(&g) > score(b)) {
            best = Some(g);
        }
    }
    Ok(best.map(to_media))
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
    query_at(client_id, secret, GAMES_URL, body)
}

fn query_at(client_id: &str, secret: &str, url: &str, body: &str) -> AppResult<String> {
    for attempt in 0..2 {
        let token = token(client_id, secret)?;
        let result = http::agent()
            .post(url)
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

// ---------------------------------------------------------------------------
// Time to beat (docs/TIME-TO-BEAT.md)

/// IGDB `game_time_to_beats`, in minutes: `hastily` = main story,
/// `normally` = main + extras, `completely` = completionist.
#[derive(Debug, Clone, PartialEq)]
pub struct IgdbTimeToBeat {
    pub main_minutes: Option<u32>,
    pub extra_minutes: Option<u32>,
    pub completionist_minutes: Option<u32>,
    pub count: Option<u32>,
}

#[derive(Deserialize)]
struct RawTimeToBeat {
    #[serde(default)]
    hastily: Option<u64>,
    #[serde(default)]
    normally: Option<u64>,
    #[serde(default)]
    completely: Option<u64>,
    #[serde(default)]
    count: Option<u32>,
}

fn minutes(secs: Option<u64>) -> Option<u32> {
    secs.filter(|s| *s > 0)
        .map(|s| ((s + 30) / 60).min(u32::MAX as u64) as u32)
}

/// `None` when IGDB has no entry for the game (an empty list).
pub fn parse_time_to_beat(body: &str) -> AppResult<Option<IgdbTimeToBeat>> {
    let rows: Vec<RawTimeToBeat> = serde_json::from_str(body)?;
    Ok(rows
        .into_iter()
        .next()
        .map(|r| IgdbTimeToBeat {
            main_minutes: minutes(r.hastily),
            extra_minutes: minutes(r.normally),
            completionist_minutes: minutes(r.completely),
            count: r.count,
        })
        .filter(|t| {
            t.main_minutes.is_some()
                || t.extra_minutes.is_some()
                || t.completionist_minutes.is_some()
        }))
}

pub fn fetch_time_to_beat(
    client_id: &str,
    secret: &str,
    igdb_id: i64,
) -> AppResult<Option<IgdbTimeToBeat>> {
    let body = query_at(
        client_id,
        secret,
        TIME_TO_BEAT_URL,
        &format!("fields hastily,normally,completely,count; where game_id = {igdb_id};"),
    )?;
    parse_time_to_beat(&body)
}

/// A game IGDB proposes when the title match is not exact.
#[derive(Debug, Clone, PartialEq, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct IgdbCandidate {
    /// IGDB ids fit in 32 bits (and TypeScript numbers).
    pub id: u32,
    pub name: String,
    pub year: Option<i32>,
    pub url: Option<String>,
}

#[derive(Deserialize)]
struct RawCandidate {
    id: i64,
    #[serde(default)]
    name: String,
    #[serde(default)]
    first_release_date: Option<i64>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    total_rating_count: Option<u32>,
}

/// The IGDB game found for a title: the one with the same title (most rated
/// first), else up to 5 candidates for the user to choose from.
#[derive(Debug, Clone, PartialEq)]
pub enum Match {
    Found(IgdbCandidate),
    Candidates(Vec<IgdbCandidate>),
}

fn candidate(c: RawCandidate) -> IgdbCandidate {
    IgdbCandidate {
        id: u32::try_from(c.id).unwrap_or(0),
        year: c
            .first_release_date
            .and_then(|t| chrono::DateTime::from_timestamp(t, 0))
            .map(|d| chrono::Datelike::year(&d)),
        name: c.name,
        url: c.url.filter(|u| u.starts_with("https://www.igdb.com/")),
    }
}

pub fn parse_match(body: &str, title: &str) -> AppResult<Match> {
    let mut games: Vec<RawCandidate> = serde_json::from_str(body)?;
    let key = match_key(title);
    games.sort_by_key(|g| std::cmp::Reverse(g.total_rating_count));
    if let Some(i) = games
        .iter()
        .position(|g| !key.is_empty() && match_key(&g.name) == key)
    {
        return Ok(Match::Found(candidate(games.swap_remove(i))));
    }
    Ok(Match::Candidates(
        games.into_iter().take(5).map(candidate).collect(),
    ))
}

const MATCH_FIELDS: &str = "fields name,first_release_date,url,total_rating_count;";

/// The IGDB game of a library game: `known_id` when set, else a title search.
pub fn find_game(
    client_id: &str,
    secret: &str,
    known_id: Option<i64>,
    title: &str,
) -> AppResult<Match> {
    if let Some(id) = known_id {
        let body = query(
            client_id,
            secret,
            &format!("{MATCH_FIELDS} where id = {id};"),
        )?;
        let games: Vec<RawCandidate> = serde_json::from_str(&body)?;
        if let Some(g) = games.into_iter().next() {
            return Ok(Match::Found(candidate(g)));
        }
    }
    let clean: String = title.chars().filter(|c| *c != '"' && *c != '\\').collect();
    let body = query(
        client_id,
        secret,
        &format!("search \"{}\"; {MATCH_FIELDS} limit 10;", clean.trim()),
    )?;
    parse_match(&body, title)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BODY: &str = r#"[
      {"id":1,"name":"Hades II","summary":"Sequel."},
      {"id":3,"name":"Hades","summary":"Obscure homonym.","total_rating_count":2},
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
    fn reads_time_to_beat_in_minutes() {
        let body = include_str!("../../tests/fixtures/igdb_time_to_beat.json");
        let t = parse_time_to_beat(body).unwrap().unwrap();
        assert_eq!(t.main_minutes, Some(2321)); // 139 240 s
        assert_eq!(t.extra_minutes, Some(4227));
        assert_eq!(t.completionist_minutes, Some(9045));
        assert_eq!(t.count, Some(13));
        assert_eq!(parse_time_to_beat("[]").unwrap(), None);
        assert_eq!(parse_time_to_beat(r#"[{"count":2}]"#).unwrap(), None);
    }

    #[test]
    fn exact_title_is_found_else_candidates() {
        let body = include_str!("../../tests/fixtures/igdb_search_candidates.json");
        match parse_match(body, "Kingdom Come: Deliverance II").unwrap() {
            Match::Found(c) => {
                assert_eq!(c.id, 119313);
                assert_eq!(c.year, Some(2025));
                assert_eq!(
                    c.url.as_deref(),
                    Some("https://www.igdb.com/games/kingdom-come-deliverance-ii")
                );
            }
            other => panic!("{other:?}"),
        }
        match parse_match(body, "Kingdom Come Deliverance 2").unwrap() {
            Match::Candidates(list) => {
                assert_eq!(list.len(), 3);
                assert_eq!(list[0].id, 2005); // most rated first
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn youtube_ids_are_checked() {
        assert!(is_youtube_id("91t0ha9x0AE"));
        assert!(is_youtube_id("a-b_c-d_e-f"));
        assert!(!is_youtube_id("short"));
        assert!(!is_youtube_id("91t0ha9x0A&"));
    }
}

/// Real IGDB call with the credentials saved in the app (ignored by default):
/// `cargo test igdb_live -- --ignored --nocapture`. Never prints the secrets.
#[cfg(all(test, windows))]
mod live {
    use super::*;
    use crate::secrets::{KeyringStore, SecretKind, SecretStore};

    #[test]
    #[ignore]
    fn igdb_live() {
        let secrets = KeyringStore::new().unwrap();
        let id = secrets
            .get(SecretKind::IgdbClientId)
            .unwrap()
            .expect("client id");
        let secret = secrets
            .get(SecretKind::IgdbClientSecret)
            .unwrap()
            .expect("client secret");
        for title in ["League of Legends", "VALORANT", "Hades", "Diablo IV"] {
            let raw = query(&id, &secret, &search_query(title)).unwrap();
            println!(
                "{title}: {} bytes, starts {}",
                raw.len(),
                &raw[..raw.len().min(300)]
            );
            let media = parse_search(&raw, title).unwrap();
            match media {
                Some(m) => println!(
                    "  -> id {} videos {} images {} summary {}",
                    m.id,
                    m.trailers.len(),
                    m.screenshots.len(),
                    m.summary.is_some()
                ),
                None => println!("  -> no exact match"),
            }
        }
    }
}
