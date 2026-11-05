//! Steam news for recently played games (SPEC § 4 "Actus"): announcements
//! promoting something else (sales, other games, bundles, soundtracks...)
//! are hidden and counted.

use serde::{Deserialize, Serialize};

use crate::error::AppResult;
use crate::http;

#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct NewsItem {
    pub game_id: String,
    pub game_title: String,
    pub title: String,
    pub url: String,
    /// RFC 3339.
    pub date: String,
    /// Short text without markup.
    pub excerpt: String,
}

#[derive(Deserialize)]
struct Envelope {
    appnews: AppNews,
}

#[derive(Deserialize)]
struct AppNews {
    #[serde(default)]
    newsitems: Vec<RawItem>,
}

#[derive(Deserialize)]
struct RawItem {
    title: String,
    url: String,
    #[serde(default)]
    contents: String,
    date: i64,
    #[serde(default)]
    feed_type: Option<i64>,
    #[serde(default)]
    tags: Vec<String>,
}

/// Announcements that advertise something else than updates of the game.
pub fn is_promotion(title: &str, tags: &[String]) -> bool {
    let t = title.to_lowercase();
    const WORDS: &[&str] = &[
        "sale",
        "soldes",
        "% off",
        "discount",
        "promo",
        "bundle",
        "soundtrack",
        "now available on",
        "free weekend",
        "week-end gratuit",
        "merch",
        "wishlist",
        "pre-order",
        "précommande",
        "crossover",
        "new game",
        "nouveau jeu",
        "announcing",
        "kickstarter",
    ];
    WORDS.iter().any(|w| t.contains(w)) || tags.iter().any(|x| x == "mod_reviewed" || x == "ads")
}

/// Removes BBCode / HTML and collapses whitespace.
pub fn excerpt(contents: &str, max: usize) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in contents.chars() {
        match c {
            '[' | '<' => in_tag = true,
            ']' | '>' => {
                in_tag = false;
                out.push(' ');
            }
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    let text = out.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() <= max {
        text
    } else {
        format!("{}…", text.chars().take(max).collect::<String>().trim_end())
    }
}

/// `(kept, hidden)` news of one game, newest first.
pub fn parse(
    body: &str,
    game_id: &str,
    game_title: &str,
) -> AppResult<(Vec<NewsItem>, Vec<NewsItem>)> {
    let env: Envelope = serde_json::from_str(body)?;
    let mut kept = Vec::new();
    let mut hidden = Vec::new();
    for item in env.appnews.newsitems {
        // Only the developer's own announcements (feed_type 1), not press articles.
        let hide = item.feed_type.is_some_and(|f| f != 1) || is_promotion(&item.title, &item.tags);
        let Some(date) = chrono::DateTime::from_timestamp(item.date, 0) else {
            continue;
        };
        let target = if hide { &mut hidden } else { &mut kept };
        target.push(NewsItem {
            game_id: game_id.to_string(),
            game_title: game_title.to_string(),
            title: item.title,
            url: item.url,
            date: date.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            excerpt: excerpt(&item.contents, 220),
        });
    }
    Ok((kept, hidden))
}

pub fn fetch(
    appid: &str,
    game_id: &str,
    game_title: &str,
) -> AppResult<(Vec<NewsItem>, Vec<NewsItem>)> {
    let mut resp = http::agent()
        .get("https://api.steampowered.com/ISteamNews/GetNewsForApp/v2/")
        .query("appid", appid)
        .query("count", "5")
        .query("maxlength", "600")
        .query("format", "json")
        .call()
        .map_err(|e| http::network_error(e, None))?;
    let body = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| http::network_error(e, None))?;
    parse(&body, game_id, game_title)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hides_promotions_and_press() {
        let body = r#"{"appnews":{"appid":1,"newsitems":[
          {"gid":"1","title":"Patch 9.0.5 Notes","url":"https://x/1","contents":"[h1]Fixes[/h1] Crash fixed <b>today</b>.","date":1793800000,"feed_type":1,"tags":["patchnotes"]},
          {"gid":"2","title":"Autumn Sale: 50% off!","url":"https://x/2","contents":"","date":1793700000,"feed_type":1},
          {"gid":"3","title":"Review on GameSite","url":"https://x/3","contents":"","date":1793600000,"feed_type":0},
          {"gid":"4","title":"Our new game is out","url":"https://x/4","contents":"","date":1793500000,"feed_type":1}
        ]}}"#;
        let (kept, hidden) = parse(body, "g1", "Total War").unwrap();
        assert_eq!(kept.len(), 1);
        assert_eq!(hidden.len(), 3);
        assert_eq!(kept[0].excerpt, "Fixes Crash fixed today .");
        assert!(kept[0].date.starts_with("2026-11"));
    }

    #[test]
    fn excerpts_are_truncated() {
        assert_eq!(excerpt("abc def ghi", 5), "abc d…");
        assert_eq!(excerpt("  a   b ", 10), "a b");
    }
}
