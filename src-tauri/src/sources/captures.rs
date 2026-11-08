//! Finding captures on disk (docs/FICHE-ONGLETS.md, docs/INTEGRATIONS.md):
//! Steam's F12 screenshots, the Xbox Game Bar folder and folders added by the
//! user. Read-only: files are listed and read, never moved or written.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, TimeZone, Utc};

use crate::db::repo::screenshots::CaptureKind;
use crate::library::merge::match_key;
use crate::sources::vdf;

/// Kind of a capture file from its extension; `None` = not a capture.
pub fn kind_of(path: &Path) -> Option<CaptureKind> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" => Some(CaptureKind::Image),
        "mp4" => Some(CaptureKind::Video),
        _ => None,
    }
}

/// UTC timestamp as stored (`YYYY-MM-DDTHH:MM:SSZ`, sorts as text).
pub fn fmt_utc(t: DateTime<Utc>) -> String {
    t.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// A wall-clock time read from a file name, in the PC's time zone.
pub fn local_to_utc(t: NaiveDateTime) -> Option<DateTime<Utc>> {
    Local
        .from_local_datetime(&t)
        .earliest()
        .map(|t| t.with_timezone(&Utc))
}

/// Steam names its screenshots `YYYYMMDDhhmmss_n.jpg` (local time).
pub fn steam_name_time(file_name: &str) -> Option<NaiveDateTime> {
    let stamp = file_name.get(..14)?;
    if !stamp.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    NaiveDateTime::parse_from_str(stamp, "%Y%m%d%H%M%S").ok()
}

fn numbers(token: &str) -> Option<Vec<u32>> {
    let parts: Vec<&str> = token.split(['-', '_', '.']).collect();
    if parts.len() != 3 || parts.iter().any(|p| p.is_empty()) {
        return None;
    }
    parts.iter().map(|p| p.parse().ok()).collect()
}

/// A Game Bar capture is named `<window title> <date> <time>`, e.g.
/// `Hades 2025-10-29 11-22-31.png` (Windows 11) or
/// `Hades 10_29_2025 11_22_31 AM.png` (older builds, locale order).
/// Returns the window title and, when the date is unambiguous (year first),
/// the local time.
pub fn parse_gamebar_name(stem: &str) -> Option<(String, Option<NaiveDateTime>)> {
    let mut tokens: Vec<&str> = stem.split_whitespace().collect();
    let pm = match tokens.last().map(|t| t.to_ascii_uppercase()) {
        Some(t) if t == "AM" || t == "PM" => {
            tokens.pop();
            Some(t == "PM")
        }
        _ => None,
    };
    let time = numbers(tokens.pop()?)?;
    let date = numbers(tokens.pop()?)?;
    let title = tokens.join(" ");
    if title.is_empty() {
        return None;
    }
    let mut hour = time[0];
    match pm {
        Some(true) if hour < 12 => hour += 12,
        Some(false) if hour == 12 => hour = 0,
        _ => {}
    }
    let when = (date[0] >= 1000)
        .then(|| NaiveDate::from_ymd_opt(date[0] as i32, date[1], date[2]))
        .flatten()
        .and_then(|d| d.and_hms_opt(hour, time[1], time[2]));
    Some((title, when))
}

/// Matches a window title or file name to a game of the library.
pub struct TitleMatcher {
    /// `(match key, game id)`, longest keys first.
    keys: Vec<(String, String)>,
}

/// Shorter titles would match too many file names ("It", "Ys").
const MIN_KEY_LEN: usize = 4;

impl TitleMatcher {
    pub fn new<'a>(games: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
        let mut keys: Vec<(String, String)> = games
            .into_iter()
            .map(|(id, title)| (match_key(title), id.to_string()))
            .filter(|(k, _)| k.chars().count() >= MIN_KEY_LEN)
            .collect();
        keys.sort_by_key(|(k, _)| std::cmp::Reverse(k.len()));
        Self { keys }
    }

    /// Same title once normalised, or a title that starts with the game's
    /// (`Hades II - Vulkan`, `Endless Space 2 2025-10-29 11-22-31`). The
    /// longest game title wins, so `Hades II` beats `Hades`.
    pub fn find(&self, text: &str) -> Option<&str> {
        let key = match_key(text);
        self.keys
            .iter()
            .find(|(k, _)| key == *k || key.starts_with(&format!("{k} ")))
            .map(|(_, id)| id.as_str())
    }
}

/// Session a capture belongs to: its time falls in `[start, end]`, limits
/// included. `sessions`: `(id, start, end)`.
pub fn session_for(
    taken: DateTime<Utc>,
    sessions: &[(String, DateTime<Utc>, DateTime<Utc>)],
) -> Option<&str> {
    sessions
        .iter()
        .find(|(_, start, end)| *start <= taken && taken <= *end)
        .map(|(id, _, _)| id.as_str())
}

/// Image and video files under `dir`, a few levels deep (recorders often make
/// one folder per game). Steam's `thumbnails` folders are skipped.
pub fn media_files(dir: &Path, depth: u32) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            let skip = path
                .file_name()
                .is_some_and(|n| n.eq_ignore_ascii_case("thumbnails"));
            if depth > 0 && !skip {
                out.extend(media_files(&path, depth - 1));
            }
        } else if kind.is_file() && kind_of(&path).is_some() {
            out.push(path);
        }
    }
    out
}

/// A screenshot taken with Steam's F12 key.
#[derive(Debug, Clone, PartialEq)]
pub struct SteamShot {
    pub appid: u64,
    pub path: PathBuf,
    pub thumb: Option<PathBuf>,
    pub taken: Option<DateTime<Utc>>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

/// `(width, height, creation unix time)` from `screenshots.vdf`.
pub type ShotInfo = (Option<u32>, Option<u32>, Option<i64>);

/// What `userdata\<account>\760\screenshots.vdf` says about each file
/// (keyed by `<appid>/screenshots/<name>`, lowercase): size and creation time.
pub fn parse_screenshots_vdf(text: &str) -> HashMap<String, ShotInfo> {
    let mut out = HashMap::new();
    let Ok(doc) = vdf::parse(text) else {
        return out;
    };
    let Some(root) = doc.get("screenshots") else {
        return out;
    };
    for (_, app) in root.entries() {
        for (_, shot) in app.entries() {
            let Some(name) = shot.get_str("filename") else {
                continue;
            };
            let num = |k: &str| shot.get_u64(k);
            out.insert(
                name.replace('\\', "/").to_lowercase(),
                (
                    num("width").and_then(|v| u32::try_from(v).ok()),
                    num("height").and_then(|v| u32::try_from(v).ok()),
                    num("creation").and_then(|v| i64::try_from(v).ok()),
                ),
            );
        }
    }
    out
}

/// Folders holding Steam screenshots: `userdata\<account>\760\remote`, one per
/// account that has some.
pub fn steam_remote_dirs(root: &Path) -> Vec<PathBuf> {
    let Ok(users) = std::fs::read_dir(root.join("userdata")) else {
        return vec![];
    };
    users
        .flatten()
        .map(|u| u.path().join("760").join("remote"))
        .filter(|p| p.is_dir())
        .collect()
}

/// Every Steam screenshot of every account on this PC.
pub fn steam_shots(root: &Path) -> Vec<SteamShot> {
    let mut out = Vec::new();
    for remote in steam_remote_dirs(root) {
        let meta = remote
            .parent()
            .and_then(|p| std::fs::read(p.join("screenshots.vdf")).ok())
            .map(|b| parse_screenshots_vdf(&String::from_utf8_lossy(&b)))
            .unwrap_or_default();
        let Ok(apps) = std::fs::read_dir(&remote) else {
            continue;
        };
        for app in apps.flatten() {
            let Some(appid) = app.file_name().to_str().and_then(|s| s.parse::<u64>().ok()) else {
                continue;
            };
            let dir = app.path().join("screenshots");
            for path in media_files(&dir, 0) {
                let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                    continue;
                };
                let info = meta
                    .get(&format!("{appid}/screenshots/{name}").to_lowercase())
                    .copied();
                let taken = info
                    .and_then(|(_, _, c)| c)
                    .and_then(|c| Utc.timestamp_opt(c, 0).single())
                    .or_else(|| steam_name_time(name).and_then(local_to_utc));
                let thumb = dir.join("thumbnails").join(name);
                out.push(SteamShot {
                    appid,
                    thumb: thumb.is_file().then_some(thumb),
                    taken,
                    width: info.and_then(|i| i.0),
                    height: info.and_then(|i| i.1),
                    path,
                });
            }
        }
    }
    out
}

/// The Game Bar folder (`%USERPROFILE%\Videos\Captures`), when it exists.
pub fn gamebar_dir() -> Option<PathBuf> {
    let home = std::env::var_os("USERPROFILE")?;
    let dir = PathBuf::from(home).join("Videos").join("Captures");
    dir.is_dir().then_some(dir)
}

/// Game of a file in an "all games" folder: by its name (Game Bar style or
/// any name starting with the title), else by its parent folders up to `root`.
pub fn match_file<'a>(matcher: &'a TitleMatcher, root: &Path, path: &Path) -> Option<&'a str> {
    let stem = path.file_stem()?.to_str()?;
    let by_name = parse_gamebar_name(stem)
        .and_then(|(title, _)| matcher.find(&title))
        .or_else(|| matcher.find(stem));
    if by_name.is_some() {
        return by_name;
    }
    let mut dir = path.parent();
    while let Some(d) = dir {
        if d == root || !d.starts_with(root) {
            break;
        }
        if let Some(found) = d
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| matcher.find(n))
        {
            return Some(found);
        }
        dir = d.parent();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utc(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    #[test]
    fn reads_the_time_in_steam_file_names() {
        let t = steam_name_time("20251029112231_1.jpg").unwrap();
        assert_eq!(t.to_string(), "2025-10-29 11:22:31");
        assert_eq!(steam_name_time("thumbs.db"), None);
        assert_eq!(steam_name_time("20251329112231_1.jpg"), None);
    }

    #[test]
    fn reads_title_and_time_in_game_bar_names() {
        let (title, t) = parse_gamebar_name("Endless Space 2 2025-10-29 11-22-31").unwrap();
        assert_eq!(title, "Endless Space 2");
        assert_eq!(t.unwrap().to_string(), "2025-10-29 11:22:31");

        let (title, t) = parse_gamebar_name("Hades 10_29_2025 9_05_00 PM").unwrap();
        assert_eq!(title, "Hades");
        assert_eq!(t, None, "month/day order depends on the locale");

        assert_eq!(parse_gamebar_name("2025-10-29 11-22-31"), None);
        assert_eq!(parse_gamebar_name("Screenshot"), None);
    }

    #[test]
    fn matches_game_bar_titles_to_games() {
        let m = TitleMatcher::new([
            ("es2", "ENDLESS™ Space 2"),
            ("hades", "Hades"),
            ("hades2", "Hades II"),
            ("it", "It"),
        ]);
        assert_eq!(m.find("Endless Space 2"), Some("es2"));
        assert_eq!(m.find("Endless Space 2 2025-10-29 11-22-31"), Some("es2"));
        assert_eq!(m.find("Hades II - Vulkan"), Some("hades2"));
        assert_eq!(m.find("Hades"), Some("hades"));
        assert_eq!(m.find("Hadesborn"), None);
        assert_eq!(m.find("It Takes Two"), None, "too short to guess");
    }

    #[test]
    fn attaches_captures_to_sessions_limits_included() {
        let sessions = vec![
            (
                "evening".to_string(),
                utc("2025-10-29T22:30:00Z"),
                utc("2025-10-30T01:15:00Z"),
            ),
            (
                "morning".to_string(),
                utc("2025-10-30T08:00:00Z"),
                utc("2025-10-30T09:00:00Z"),
            ),
        ];
        let at = |s| session_for(utc(s), &sessions);
        assert_eq!(at("2025-10-29T22:30:00Z"), Some("evening"));
        assert_eq!(
            at("2025-10-30T00:10:00Z"),
            Some("evening"),
            "after midnight"
        );
        assert_eq!(at("2025-10-30T09:00:00Z"), Some("morning"));
        assert_eq!(at("2025-10-30T09:00:01Z"), None);
    }

    #[test]
    fn reads_steam_screenshots_vdf() {
        let text = r#""screenshots"
{
	"392110"
	{
		"0"
		{
			"type"		"1"
			"filename"		"392110/screenshots/20251029112231_1.jpg"
			"width"		"2560"
			"height"		"1440"
			"creation"		"1761733351"
		}
	}
	"shortcutnames"
	{
	}
}"#;
        let meta = parse_screenshots_vdf(text);
        assert_eq!(
            meta.get("392110/screenshots/20251029112231_1.jpg"),
            Some(&(Some(2560), Some(1440), Some(1_761_733_351)))
        );
    }

    #[test]
    fn finds_steam_shots_and_files_by_name_or_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let shots = tmp
            .path()
            .join(r"userdata\72883778\760\remote\392110\screenshots");
        std::fs::create_dir_all(shots.join("thumbnails")).unwrap();
        std::fs::write(shots.join("20251029112231_1.jpg"), b"x").unwrap();
        std::fs::write(shots.join(r"thumbnails\20251029112231_1.jpg"), b"x").unwrap();
        std::fs::write(shots.join("notes.txt"), b"x").unwrap();
        let found = steam_shots(tmp.path());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].appid, 392110);
        assert!(found[0].thumb.is_some());
        assert!(found[0].taken.is_some(), "time from the file name");

        let m = TitleMatcher::new([("hades", "Hades")]);
        let root = tmp.path().join("Videos");
        let file = root.join(r"Hades\clip_0001.mp4");
        assert_eq!(match_file(&m, &root, &file), Some("hades"));
        assert_eq!(
            match_file(&m, &root, &root.join("Hades 2025-01-02 10-00-00.png")),
            Some("hades")
        );
        assert_eq!(match_file(&m, &root, &root.join("Desktop.png")), None);
    }
}
