//! Genres and player modes: Steam store (French) and GOG Galaxy (English)
//! names normalized to one French list, so filters group both platforms.

use serde::Deserialize;

use crate::error::{AppError, AppResult};
use crate::http;

/// Canonical French genre name for a Steam or GOG genre.
pub fn normalize_genre(name: &str) -> Option<String> {
    let n = name.trim().to_lowercase();
    if n.is_empty() {
        return None;
    }
    let canonical = match n.as_str() {
        "role-playing (rpg)" | "rpg" | "role-playing" | "jeu de rôle" => "RPG",
        "adventure" | "aventure" => "Aventure",
        "action" => "Action",
        "strategy" | "stratégie" | "real time strategy (rts)" | "turn-based strategy (tbs)" => {
            "Stratégie"
        }
        "tactical" | "tactique" => "Tactique",
        "shooter" | "tir" => "Tir",
        "simulator" | "simulation" => "Simulation",
        "indie" | "indépendant" => "Indépendant",
        "racing" | "course" | "courses" => "Course",
        "sport" | "sports" => "Sport",
        "puzzle" | "réflexion" => "Réflexion",
        "platform" | "plateforme" | "plates-formes" => "Plateforme",
        "fighting" | "combat" => "Combat",
        "massively multiplayer" | "massivement multijoueur" | "mmo" => "MMO",
        "casual" | "occasionnel" => "Occasionnel",
        "point-and-click" => "Point & click",
        "hack and slash/beat 'em up" => "Beat'em all",
        "arcade" => "Arcade",
        "music" | "musique" => "Musique",
        "visual novel" => "Visual novel",
        "card & board game" | "jeu de cartes et de plateau" => "Cartes & plateau",
        "early access" | "accès anticipé" => "Accès anticipé",
        "free to play" | "free-to-play" | "gratuit" => "Gratuit",
        // Not genres: tools and software categories.
        "utilities"
        | "utilitaires"
        | "software training"
        | "design & illustration"
        | "animation & modeling"
        | "audio production"
        | "video production"
        | "photo editing"
        | "web publishing"
        | "education"
        | "éducation" => return None,
        _ => return Some(name.trim().to_string()),
    };
    Some(canonical.to_string())
}

/// Player modes from Steam store category ids.
pub fn modes_from_categories(ids: &[u32]) -> Vec<String> {
    let mut modes = Vec::new();
    let mut add = |m: &str| {
        if !modes.iter().any(|x| x == m) {
            modes.push(m.to_string());
        }
    };
    for id in ids {
        match id {
            2 => add("Solo"),
            1 | 36 | 37 | 47 | 49 => add("Multijoueur"),
            9 | 38 | 48 => add("Coopération"),
            24 | 39 | 44 => add("Écran partagé"),
            _ => {}
        }
    }
    modes
}

#[derive(Deserialize)]
struct Named {
    description: String,
}

#[derive(Deserialize)]
struct Category {
    id: u32,
}

#[derive(Deserialize)]
struct AppData {
    #[serde(default)]
    genres: Vec<Named>,
    #[serde(default)]
    categories: Vec<Category>,
}

#[derive(Deserialize)]
struct AppEntry {
    success: bool,
    data: Option<AppData>,
}

/// `(genres, modes)` from a store `appdetails` response; `None` when the
/// store has no page for the app (removed games, tools).
pub fn parse_appdetails(body: &str, appid: &str) -> AppResult<Option<(Vec<String>, Vec<String>)>> {
    let map: std::collections::HashMap<String, AppEntry> = serde_json::from_str(body)?;
    let Some(entry) = map.get(appid) else {
        return Ok(None);
    };
    let Some(data) = entry.data.as_ref().filter(|_| entry.success) else {
        return Ok(None);
    };
    let mut genres: Vec<String> = Vec::new();
    for g in &data.genres {
        if let Some(n) = normalize_genre(&g.description) {
            if !genres.contains(&n) {
                genres.push(n);
            }
        }
    }
    let ids: Vec<u32> = data.categories.iter().map(|c| c.id).collect();
    Ok(Some((genres, modes_from_categories(&ids))))
}

/// Store data for one Steam app (no key needed; ~200 requests / 5 min).
pub fn fetch_steam(appid: &str) -> AppResult<Option<(Vec<String>, Vec<String>)>> {
    let mut resp = http::agent()
        .get("https://store.steampowered.com/api/appdetails")
        .query("appids", appid)
        .query("l", "french")
        .query("filters", "genres,categories")
        .call()
        .map_err(|e| match e {
            ureq::Error::StatusCode(429) => AppError::Network {
                status: Some(429),
                message: "rate limited".into(),
            },
            other => http::network_error(other, None),
        })?;
    let body = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| http::network_error(e, None))?;
    parse_appdetails(&body, appid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_steam_and_gog_genres() {
        assert_eq!(
            normalize_genre("Role-playing (RPG)").as_deref(),
            Some("RPG")
        );
        assert_eq!(normalize_genre("RPG").as_deref(), Some("RPG"));
        assert_eq!(normalize_genre("Adventure").as_deref(), Some("Aventure"));
        assert_eq!(
            normalize_genre("Indépendant").as_deref(),
            Some("Indépendant")
        );
        assert_eq!(
            normalize_genre("Massivement multijoueur").as_deref(),
            Some("MMO")
        );
        assert_eq!(normalize_genre("Utilitaires"), None);
        assert_eq!(normalize_genre("Roguelike").as_deref(), Some("Roguelike"));
        assert_eq!(normalize_genre("  "), None);
    }

    #[test]
    fn maps_categories_to_modes() {
        assert_eq!(
            modes_from_categories(&[2, 1, 9, 22, 28]),
            ["Solo", "Multijoueur", "Coopération"]
        );
        assert_eq!(
            modes_from_categories(&[38, 39, 36]),
            ["Coopération", "Écran partagé", "Multijoueur"]
        );
        assert!(modes_from_categories(&[22]).is_empty());
    }

    #[test]
    fn parses_store_responses() {
        let body = r#"{"374320":{"success":true,"data":{"genres":[{"id":"1","description":"Action"}],
            "categories":[{"id":2,"description":"Solo"},{"id":1,"description":"Multijoueur"}]}}}"#;
        let (genres, modes) = parse_appdetails(body, "374320").unwrap().unwrap();
        assert_eq!(genres, ["Action"]);
        assert_eq!(modes, ["Solo", "Multijoueur"]);
        assert_eq!(
            parse_appdetails(r#"{"1":{"success":false}}"#, "1").unwrap(),
            None
        );
        assert_eq!(
            parse_appdetails(r#"{"1":{"success":true,"data":{}}}"#, "1").unwrap(),
            Some((vec![], vec![]))
        );
    }
}
