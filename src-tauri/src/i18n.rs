//! UI language for the few texts the backend shows itself (notification-area
//! menu) and for the language asked to Steam (store descriptions, achievement
//! names). The frontend owns the choice and sends it with `language_set`.

use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};

use crate::db::repo::settings;
use crate::error::AppResult;

pub const LANGUAGE_KEY: &str = "app.language";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Fr,
    En,
}

static ENGLISH: AtomicBool = AtomicBool::new(false);

pub fn current() -> Language {
    if ENGLISH.load(Ordering::Relaxed) {
        Language::En
    } else {
        Language::Fr
    }
}

pub fn set(lang: Language) {
    ENGLISH.store(lang == Language::En, Ordering::Relaxed);
}

/// Reads the saved language at startup (French when never set).
pub fn load(conn: &rusqlite::Connection) -> AppResult<Language> {
    let lang = settings::get::<Language>(conn, LANGUAGE_KEY)?.unwrap_or(Language::Fr);
    set(lang);
    Ok(lang)
}

pub fn save(conn: &rusqlite::Connection, lang: Language) -> AppResult<()> {
    settings::set(conn, LANGUAGE_KEY, &lang)?;
    set(lang);
    Ok(())
}

/// Steam's `l=` parameter and the key of localized texts in Steam schemas.
pub fn steam_language() -> &'static str {
    match current() {
        Language::Fr => "french",
        Language::En => "english",
    }
}

/// Backend texts, by language.
pub struct Texts {
    pub no_game: &'static str,
    pub in_game: &'static str,
    pub show: &'static str,
    pub couch: &'static str,
    pub quit: &'static str,
}

pub fn texts(lang: Language) -> &'static Texts {
    match lang {
        Language::Fr => &Texts {
            no_game: "Aucun jeu en cours",
            in_game: "En jeu :",
            show: "Afficher Ignite",
            couch: "Mode Canapé",
            quit: "Quitter",
        },
        Language::En => &Texts {
            no_game: "No game running",
            in_game: "Playing:",
            show: "Show Ignite",
            couch: "Couch mode",
            quit: "Quit",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;

    #[test]
    fn saves_and_reloads_the_language() {
        let db = Db::open_in_memory().unwrap();
        let conn = db.conn();
        assert_eq!(load(&conn).unwrap(), Language::Fr);
        save(&conn, Language::En).unwrap();
        assert_eq!(steam_language(), "english");
        set(Language::Fr);
        assert_eq!(load(&conn).unwrap(), Language::En);
        set(Language::Fr);
    }
}
