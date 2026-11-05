//! GOG Galaxy source, read from Galaxy's own SQLite database.
//!
//! The database is never opened in place: `galaxy-2.0.db` and its `-wal` /
//! `-shm` companions are copied to a temporary folder first (Galaxy keeps it
//! open and recent changes live in the WAL file). Only `gog_*` releases are
//! read: entries of other launchers connected inside Galaxy are ignored, they
//! come from their own sources (no double counting).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension};

use crate::db::repo::achievements::AchievementRow;
use crate::error::{AppError, AppResult};
use crate::models::SourceKind;
use crate::sources::{
    ArtworkCandidate, ArtworkKind, ArtworkSource, DetectedGame, ScanResult, Source, SourceStatus,
};

pub const DB_FILE: &str = "galaxy-2.0.db";

/// `C:\ProgramData\GOG.com\Galaxy\storage\galaxy-2.0.db`, if present.
pub fn locate_db() -> Option<PathBuf> {
    let base = std::env::var_os("PROGRAMDATA").map(PathBuf::from)?;
    let path = base
        .join("GOG.com")
        .join("Galaxy")
        .join("storage")
        .join(DB_FILE);
    path.is_file().then_some(path)
}

/// `GalaxyClient.exe`, used to launch games the way Galaxy does.
pub fn locate_client() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        use winreg::enums::HKEY_LOCAL_MACHINE;
        use winreg::RegKey;
        let key = RegKey::predef(HKEY_LOCAL_MACHINE)
            .open_subkey(r"SOFTWARE\WOW6432Node\GOG.com\GalaxyClient")
            .ok()?;
        let dir: String = key.open_subkey("paths").ok()?.get_value("client").ok()?;
        let exe: String = key
            .get_value("clientExecutable")
            .unwrap_or_else(|_| "GalaxyClient.exe".into());
        let path = PathBuf::from(dir).join(exe);
        path.is_file().then_some(path)
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// Copies the database and its WAL/SHM files into a fresh temporary folder.
fn copy_db(db: &Path) -> AppResult<(tempdir::TempDir, PathBuf)> {
    let dir = tempdir::TempDir::new()?;
    let target = dir.path().join(DB_FILE);
    std::fs::copy(db, &target)?;
    for suffix in ["-wal", "-shm"] {
        let side = PathBuf::from(format!("{}{suffix}", db.display()));
        if side.is_file() {
            std::fs::copy(
                &side,
                PathBuf::from(format!("{}{suffix}", target.display())),
            )?;
        }
    }
    Ok((dir, target))
}

/// Minimal self-cleaning temporary directory (avoids a dependency).
mod tempdir {
    use std::path::{Path, PathBuf};

    pub struct TempDir(PathBuf);

    impl TempDir {
        pub fn new() -> std::io::Result<Self> {
            let path = std::env::temp_dir().join(format!("ludotheque-gog-{}", crate::db::new_id()));
            std::fs::create_dir_all(&path)?;
            Ok(Self(path))
        }

        pub fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

/// One GOG game read from the database.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GogGame {
    pub product_id: String,
    pub title: String,
    pub minutes: Option<i64>,
    pub last_played: Option<String>,
    pub install_path: Option<String>,
    pub exe_path: Option<String>,
    pub cover_url: Option<String>,
    pub hero_url: Option<String>,
    /// DLC variants Galaxy lists as separate releases.
    pub probably_dlc: bool,
    /// `None` when Galaxy has no achievement data for the game.
    pub achievements: Option<Vec<AchievementRow>>,
}

/// Galaxy dates are `YYYY-MM-DD HH:MM:SS` in UTC.
fn galaxy_date(raw: &str) -> Option<String> {
    chrono::NaiveDateTime::parse_from_str(raw.trim(), "%Y-%m-%d %H:%M:%S")
        .ok()
        .map(|d| {
            d.and_utc()
                .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
        })
}

/// Titles such as `dlc_13_a` or `Temerian Armor Set [A]`: DLC variants that
/// the base game does not list in its `dlcs` piece.
pub fn looks_like_dlc_variant(title: &str) -> bool {
    let t = title.trim();
    let lower = t.to_lowercase();
    lower.starts_with("dlc_")
        || t.ends_with("[A]")
        || lower.ends_with("(dlc)")
        || lower.contains("assets pack")
}

fn piece(conn: &Connection, key: &str, kind: &str) -> AppResult<Option<serde_json::Value>> {
    let raw: Option<String> = conn
        .query_row(
            "SELECT gp.value FROM GamePieces gp JOIN GamePieceTypes t ON t.id = gp.gamePieceTypeId \
             WHERE gp.releaseKey = ?1 AND t.type = ?2",
            [key, kind],
            |r| r.get(0),
        )
        .optional()?;
    Ok(raw.and_then(|r| serde_json::from_str(&r).ok()))
}

fn piece_str(conn: &Connection, key: &str, kind: &str, field: &str) -> AppResult<Option<String>> {
    Ok(piece(conn, key, kind)?
        .and_then(|v| v.get(field).and_then(|f| f.as_str()).map(str::to_string))
        .filter(|s| !s.trim().is_empty()))
}

fn has_table(conn: &Connection, name: &str) -> AppResult<bool> {
    Ok(conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [name],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

/// Language ids used for achievement texts: French first, then English.
fn language_order(conn: &Connection) -> AppResult<Vec<i64>> {
    if !has_table(conn, "Languages")? {
        return Ok(Vec::new());
    }
    let mut ids = Vec::new();
    for code in ["fr-FR", "fr", "en-US", "en"] {
        if let Some(id) = conn
            .query_row("SELECT id FROM Languages WHERE code = ?1", [code], |r| {
                r.get(0)
            })
            .optional()?
        {
            ids.push(id);
        }
    }
    Ok(ids)
}

/// apikey, rarity, icon, isUnlocked, unlockTime.
type AchievementBase = (
    String,
    Option<f64>,
    Option<String>,
    Option<i64>,
    Option<String>,
);

fn achievements(
    conn: &Connection,
    key: &str,
    langs: &[i64],
) -> AppResult<Option<Vec<AchievementRow>>> {
    if !has_table(conn, "Achievements")? {
        return Ok(None);
    }
    let mut stmt = conn.prepare(
        "SELECT a.apikey, a.rarity, a.imageUnlockedUrl, ua.isUnlocked, ua.unlockTime \
         FROM Achievements a LEFT JOIN UserAchievements ua \
           ON ua.gameReleaseKey = a.gameReleaseKey AND ua.apikey = a.apikey \
         WHERE a.gameReleaseKey = ?1 ORDER BY a.apikey",
    )?;
    let base: Vec<AchievementBase> = stmt
        .query_map([key], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })?
        .collect::<Result<_, _>>()?;
    if base.is_empty() {
        return Ok(None);
    }
    let mut texts: HashMap<String, (Option<String>, Option<String>)> = HashMap::new();
    if has_table(conn, "LocalizedAchievements")? {
        let mut stmt = conn.prepare(
            "SELECT apikey, name, description, languageId FROM LocalizedAchievements WHERE gameReleaseKey = ?1",
        )?;
        let rows: Vec<(String, Option<String>, Option<String>, i64)> = stmt
            .query_map([key], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
            .collect::<Result<_, _>>()?;
        let rank = |lang: i64| langs.iter().position(|l| *l == lang).unwrap_or(usize::MAX);
        let mut best: HashMap<String, usize> = HashMap::new();
        for (api, name, desc, lang) in rows {
            let r = rank(lang);
            if best.get(&api).is_none_or(|b| r < *b) {
                best.insert(api.clone(), r);
                texts.insert(api, (name, desc));
            }
        }
    }
    Ok(Some(
        base.into_iter()
            .map(|(api, rarity, icon, unlocked, time)| {
                let (name, description) = texts.remove(&api).unwrap_or((None, None));
                AchievementRow {
                    name,
                    description: description.filter(|d| !d.is_empty()),
                    icon_url: icon,
                    unlocked_at: (unlocked == Some(1))
                        .then(|| time.as_deref().and_then(galaxy_date))
                        .flatten(),
                    global_percent: rarity,
                    api_name: api,
                }
            })
            .collect(),
    ))
}

/// Reads the owned GOG games (DLCs listed by their base game excluded).
pub fn read_library(conn: &Connection) -> AppResult<Vec<GogGame>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT releaseKey FROM LibraryReleases WHERE releaseKey LIKE 'gog\\_%' ESCAPE '\\'",
    )?;
    let keys: Vec<String> = stmt
        .query_map([], |r| r.get(0))?
        .collect::<Result<_, _>>()?;

    let mut dlcs: HashSet<String> = HashSet::new();
    for key in &keys {
        if let Some(list) = piece(conn, key, "dlcs")?.and_then(|v| v.get("dlcs").cloned()) {
            for d in list.as_array().into_iter().flatten() {
                if let Some(s) = d.as_str() {
                    dlcs.insert(s.to_string());
                }
            }
        }
    }

    let mut installs: HashMap<String, String> = HashMap::new();
    if has_table(conn, "InstalledBaseProducts")? {
        let mut stmt =
            conn.prepare("SELECT productId, installationPath FROM InstalledBaseProducts")?;
        for row in stmt.query_map([], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?))
        })? {
            let (id, path) = row?;
            if let Some(p) = path.filter(|p| !p.is_empty()) {
                installs.insert(format!("gog_{id}"), p);
            }
        }
    }
    let langs = language_order(conn)?;

    let mut games = Vec::new();
    for key in keys.into_iter().filter(|k| !dlcs.contains(k)) {
        let Some(title) = piece_str(conn, &key, "title", "title")?.or(piece_str(
            conn,
            &key,
            "originalTitle",
            "title",
        )?) else {
            continue;
        };
        let minutes: Option<i64> = conn
            .query_row(
                "SELECT minutesInGame FROM GameTimes WHERE releaseKey = ?1",
                [&key],
                |r| r.get(0),
            )
            .optional()?;
        let last: Option<String> = conn
            .query_row(
                "SELECT MAX(lastPlayedDate) FROM LastPlayedDates WHERE gameReleaseKey = ?1",
                [&key],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        let exe: Option<String> = conn
            .query_row(
                "SELECT p.executablePath FROM PlayTasks t JOIN PlayTaskLaunchParameters p ON p.playTaskId = t.id \
                 WHERE t.gameReleaseKey = ?1 ORDER BY t.isPrimary DESC, t.\"order\" LIMIT 1",
                [&key],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        let install_path = installs.get(&key).cloned();
        games.push(GogGame {
            product_id: key.trim_start_matches("gog_").to_string(),
            probably_dlc: looks_like_dlc_variant(&title),
            title,
            minutes: minutes.filter(|m| *m > 0),
            last_played: last.as_deref().and_then(galaxy_date),
            exe_path: exe.filter(|_| install_path.is_some()),
            install_path,
            cover_url: piece_str(conn, &key, "originalImages", "verticalCover")?,
            hero_url: piece_str(conn, &key, "originalImages", "background")?,
            achievements: achievements(conn, &key, &langs)?,
        });
    }
    games.sort_by(|a, b| a.product_id.cmp(&b.product_id));
    Ok(games)
}

pub fn to_detected(g: GogGame) -> DetectedGame {
    let artwork = [
        (ArtworkKind::Cover, &g.cover_url),
        (ArtworkKind::Hero, &g.hero_url),
    ]
    .into_iter()
    .filter_map(|(kind, url)| {
        url.clone().map(|u| ArtworkCandidate {
            kind,
            sources: vec![ArtworkSource::Url(u)],
        })
    })
    .collect();
    DetectedGame {
        external_id: g.product_id,
        title: g.title,
        installed: g.install_path.is_some(),
        install_path: g.install_path,
        exe_path: g.exe_path,
        size_bytes: None,
        launch_uri: None,
        playtime_minutes: g.minutes,
        last_played: g.last_played,
        artwork,
        hide_on_import: g.probably_dlc,
        achievements: g.achievements,
    }
}

pub struct GogSource {
    db: Option<PathBuf>,
}

impl GogSource {
    pub fn new(db: Option<PathBuf>) -> Self {
        Self { db }
    }
}

impl Source for GogSource {
    fn kind(&self) -> SourceKind {
        SourceKind::Gog
    }

    fn status(&self) -> SourceStatus {
        match &self.db {
            Some(_) => SourceStatus::Connected,
            None => SourceStatus::NotInstalled,
        }
    }

    fn scan(&self) -> AppResult<ScanResult> {
        let db = self
            .db
            .as_ref()
            .ok_or_else(|| AppError::Invalid("GOG Galaxy not installed".into()))?;
        let (_guard, copy) = copy_db(db)?;
        let conn = Connection::open(&copy)?;
        let games = read_library(&conn)?.into_iter().map(to_detected).collect();
        Ok(ScanResult {
            kind: SourceKind::Gog,
            status: SourceStatus::Connected,
            games,
            // Galaxy lists every owned game, installed or not.
            complete_ownership: true,
            warnings: Vec::new(),
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A small Galaxy-like database with the tables and pieces we read.
    pub(crate) fn fixture() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE LibraryReleases (id INTEGER, userId INTEGER, releaseKey TEXT);
            CREATE TABLE GamePieceTypes (id INTEGER, type TEXT);
            CREATE TABLE GamePieces (releaseKey TEXT, gamePieceTypeId INTEGER, userId INTEGER, value TEXT, languageId INTEGER);
            CREATE TABLE GameTimes (userId INTEGER, releaseKey TEXT, minutesInGame INTEGER);
            CREATE TABLE LastPlayedDates (userId INTEGER, gameReleaseKey TEXT, lastPlayedDate TEXT);
            CREATE TABLE InstalledBaseProducts (productId INTEGER, installationPath TEXT);
            CREATE TABLE PlayTasks (id INTEGER, gameReleaseKey TEXT, userId INTEGER, "order" INTEGER, typeId INTEGER, isPrimary INTEGER);
            CREATE TABLE PlayTaskLaunchParameters (playTaskId INTEGER, executablePath TEXT, commandLineArgs TEXT, label TEXT);
            CREATE TABLE Languages (id INTEGER, name TEXT, code TEXT);
            CREATE TABLE Achievements (gameReleaseKey TEXT, apikey TEXT, imageUnlockedUrl TEXT, rarity REAL);
            CREATE TABLE UserAchievements (gameReleaseKey TEXT, apikey TEXT, isUnlocked INTEGER, unlockTime TEXT);
            CREATE TABLE LocalizedAchievements (gameReleaseKey TEXT, apikey TEXT, name TEXT, description TEXT, languageId INTEGER);

            INSERT INTO GamePieceTypes VALUES (128, 'title'), (50, 'originalTitle'), (45, 'dlcs'), (47, 'originalImages');
            INSERT INTO LibraryReleases VALUES
              (1, 1, 'gog_100'), (2, 1, 'gog_200'), (3, 1, 'gog_300'), (4, 1, 'gog_400'), (5, 1, 'steam_730');
            INSERT INTO GamePieces VALUES
              ('gog_100', 128, 1, '{"title":"The Witcher 3: Wild Hunt"}', NULL),
              ('gog_100', 45, 1, '{"dlcs":["gog_200"]}', NULL),
              ('gog_100', 47, 1, '{"verticalCover":"https://images.gog.com/c.webp","background":"https://images.gog.com/b.webp"}', NULL),
              ('gog_200', 128, 1, '{"title":"Temerian Armor Set"}', NULL),
              ('gog_300', 50, 1, '{"title":"Beneath a Steel Sky"}', NULL),
              ('gog_400', 128, 1, '{"title":"dlc_13_a"}', NULL),
              ('steam_730', 128, 1, '{"title":"Counter-Strike 2"}', NULL);
            INSERT INTO GameTimes VALUES (1, 'gog_100', 3188), (1, 'gog_300', 0), (1, 'steam_730', 999);
            INSERT INTO LastPlayedDates VALUES (1, 'gog_100', '2026-10-10 10:02:34');
            INSERT INTO InstalledBaseProducts VALUES (100, 'C:\Games\The Witcher 3');
            INSERT INTO PlayTasks VALUES (1, 'gog_100', 1, 1, 1, 1);
            INSERT INTO PlayTaskLaunchParameters VALUES (1, 'C:\Games\The Witcher 3\REDprelauncher.exe', '', 'Play');
            INSERT INTO Languages VALUES (16, 'English', 'en-US'), (24, 'French', 'fr-FR');
            INSERT INTO Achievements VALUES ('gog_100', 'BOOKWORM', 'https://x/a.png', 30.5), ('gog_100', 'ALL_IN', NULL, 12.0);
            INSERT INTO UserAchievements VALUES ('gog_100', 'BOOKWORM', 1, '2016-11-03 11:35:52'), ('gog_100', 'ALL_IN', 0, NULL);
            INSERT INTO LocalizedAchievements VALUES
              ('gog_100', 'BOOKWORM', 'Bookworm', 'Read 100 books', 16),
              ('gog_100', 'BOOKWORM', 'Rat de bibliothèque', 'Lire 100 livres', 24),
              ('gog_100', 'ALL_IN', 'All In', '', 16);
            "#,
        )
        .unwrap();
        conn
    }

    #[test]
    fn reads_games_skips_dlcs_and_other_launchers() {
        let games = read_library(&fixture()).unwrap();
        let titles: Vec<_> = games
            .iter()
            .map(|g| (g.product_id.as_str(), g.title.as_str()))
            .collect();
        assert_eq!(
            titles,
            [
                ("100", "The Witcher 3: Wild Hunt"),
                ("300", "Beneath a Steel Sky"),
                ("400", "dlc_13_a")
            ]
        );
        let w3 = &games[0];
        assert_eq!(w3.minutes, Some(3188));
        assert_eq!(w3.last_played.as_deref(), Some("2026-10-10T10:02:34Z"));
        assert_eq!(w3.install_path.as_deref(), Some(r"C:\Games\The Witcher 3"));
        assert_eq!(
            w3.exe_path.as_deref(),
            Some(r"C:\Games\The Witcher 3\REDprelauncher.exe")
        );
        assert_eq!(
            w3.cover_url.as_deref(),
            Some("https://images.gog.com/c.webp")
        );
        assert!(!w3.probably_dlc);
        // Never played: no counter; not installed: no exe.
        assert_eq!(games[1].minutes, None);
        assert_eq!(games[1].exe_path, None);
        assert!(games[2].probably_dlc);
    }

    #[test]
    fn reads_achievements_in_french_when_available() {
        let games = read_library(&fixture()).unwrap();
        let ach = games[0].achievements.as_ref().unwrap();
        assert_eq!(ach.len(), 2);
        let all_in = &ach[0];
        assert_eq!(all_in.name.as_deref(), Some("All In"));
        assert_eq!(all_in.description, None);
        assert_eq!(all_in.unlocked_at, None);
        let book = &ach[1];
        assert_eq!(book.name.as_deref(), Some("Rat de bibliothèque"));
        assert_eq!(book.unlocked_at.as_deref(), Some("2016-11-03T11:35:52Z"));
        assert_eq!(book.global_percent, Some(30.5));
        assert_eq!(games[1].achievements, None);
    }

    #[test]
    fn detects_dlc_variants() {
        assert!(looks_like_dlc_variant("dlc_9_a"));
        assert!(looks_like_dlc_variant("Alternative Look for Yennefer [A]"));
        assert!(looks_like_dlc_variant("4k graphic assets pack (preview)"));
        assert!(!looks_like_dlc_variant("Dragonsphere"));
        assert!(!looks_like_dlc_variant("DLC Quest"));
    }

    #[test]
    fn missing_tables_are_tolerated() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE LibraryReleases (id INTEGER, userId INTEGER, releaseKey TEXT);
             CREATE TABLE GamePieceTypes (id INTEGER, type TEXT);
             CREATE TABLE GamePieces (releaseKey TEXT, gamePieceTypeId INTEGER, value TEXT);
             CREATE TABLE GameTimes (releaseKey TEXT, minutesInGame INTEGER);
             CREATE TABLE LastPlayedDates (gameReleaseKey TEXT, lastPlayedDate TEXT);
             CREATE TABLE PlayTasks (id INTEGER, gameReleaseKey TEXT, \"order\" INTEGER, isPrimary INTEGER);
             CREATE TABLE PlayTaskLaunchParameters (playTaskId INTEGER, executablePath TEXT);
             INSERT INTO GamePieceTypes VALUES (128, 'title');
             INSERT INTO LibraryReleases VALUES (1, 1, 'gog_1');
             INSERT INTO GamePieces VALUES ('gog_1', 128, '{\"title\":\"Teenagent\"}');",
        )
        .unwrap();
        let games = read_library(&conn).unwrap();
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].achievements, None);
    }

    #[test]
    fn copies_the_database_with_its_wal() {
        let dir = std::env::temp_dir().join(format!("ludotheque-gog-src-{}", crate::db::new_id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join(DB_FILE);
        {
            let conn = Connection::open(&db).unwrap();
            conn.pragma_update(None, "journal_mode", "WAL").unwrap();
            conn.execute_batch("CREATE TABLE t (x INTEGER); INSERT INTO t VALUES (42);")
                .unwrap();
            // Keep the connection open so data stays in the WAL, like Galaxy.
            let (_tmp, copy) = copy_db(&db).unwrap();
            let read = Connection::open(&copy).unwrap();
            let x: i64 = read.query_row("SELECT x FROM t", [], |r| r.get(0)).unwrap();
            assert_eq!(x, 42);
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Reads the real Galaxy database of this PC (copy only).
    /// Run with `cargo test gog -- --ignored --nocapture`.
    #[test]
    #[ignore = "reads the local GOG Galaxy database"]
    fn reads_the_real_galaxy_database() {
        let Some(db) = locate_db() else { return };
        let result = GogSource::new(Some(db)).scan().unwrap();
        for g in &result.games {
            println!(
                "{} | {} | installed={} | min={:?} | hidden={} | ach={:?}",
                g.external_id,
                g.title,
                g.installed,
                g.playtime_minutes,
                g.hide_on_import,
                g.achievements.as_ref().map(|a| (
                    a.iter().filter(|x| x.unlocked_at.is_some()).count(),
                    a.len()
                ))
            );
        }
        assert!(!result.games.is_empty());
    }

    #[test]
    fn missing_galaxy_is_not_installed() {
        let src = GogSource::new(None);
        assert_eq!(src.status(), SourceStatus::NotInstalled);
    }
}
