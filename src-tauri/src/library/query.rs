//! Library listing: game summaries, filters, sorting and sidebar counters.

use std::collections::{BTreeSet, HashMap};

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::db::repo::{achievements, artwork, game_sources, games, goals, sessions};
use crate::error::AppResult;
use crate::library::playtime;
use crate::models::{GameStatus, SourceKind};

#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GameSummary {
    pub id: String,
    pub title: String,
    #[serde(skip)]
    pub sort_title: String,
    pub status: GameStatus,
    pub favorite: bool,
    pub hidden: bool,
    pub installed: bool,
    /// Distinct platforms, sorted.
    pub sources: Vec<SourceKind>,
    pub playtime_minutes: u32,
    /// RFC 3339.
    pub last_played: Option<String>,
    pub added_at: String,
    /// Absolute paths in the app's artwork cache (served via `asset:`).
    pub cover: Option<String>,
    pub header: Option<String>,
    /// Achievements (cached), when the platform provides them.
    pub achievements: Option<Progress>,
    /// Personal goals, when the user defined some.
    pub goals: Option<Progress>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, specta::Type)]
pub struct Progress {
    pub done: u32,
    pub total: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, specta::Type)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum LibraryView {
    All,
    Installed,
    Favorites,
    /// Hidden games only (playtests, betas, games hidden by the user).
    Hidden,
    Status(GameStatus),
    Source(SourceKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum SortKey {
    LastPlayed,
    Name,
    Playtime,
    Added,
}

#[derive(Debug, Clone, PartialEq, Deserialize, specta::Type)]
pub struct LibraryQuery {
    pub view: LibraryView,
    pub text: Option<String>,
    pub sort: SortKey,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, specta::Type)]
pub struct StatusCounts {
    pub todo: u32,
    pub playing: u32,
    pub done: u32,
    pub dropped: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
pub struct SourceCount {
    pub kind: SourceKind,
    pub count: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, specta::Type)]
pub struct LibraryCounts {
    pub all: u32,
    pub installed: u32,
    pub favorites: u32,
    pub hidden: u32,
    pub status: StatusCounts,
    pub sources: Vec<SourceCount>,
}

fn later(a: Option<String>, b: Option<String>) -> Option<String> {
    match (a, b) {
        (Some(a), Some(b)) => Some(if b > a { b } else { a }),
        (a, b) => a.or(b),
    }
}

/// Builds a summary for every game (hidden ones included).
pub fn summaries(conn: &Connection) -> AppResult<Vec<GameSummary>> {
    let all_games = games::list(conn, true)?;
    let mut sources_by_game: HashMap<String, Vec<game_sources::GameSource>> = HashMap::new();
    for s in game_sources::list_all(conn)? {
        sources_by_game
            .entry(s.game_id.clone())
            .or_default()
            .push(s);
    }
    let mut sessions_by_game: HashMap<String, Vec<sessions::SessionSum>> = HashMap::new();
    for s in sessions::sums(conn)? {
        sessions_by_game
            .entry(s.game_id.clone())
            .or_default()
            .push(s);
    }
    let mut art = artwork::all(conn)?;
    let ach = achievements::progress_by_game(conn)?;
    let goal_progress: HashMap<String, Progress> = goals::progress(conn)?
        .into_iter()
        .map(|(g, done, total)| (g, Progress { done, total }))
        .collect();

    Ok(all_games
        .into_iter()
        .map(|g| {
            let srcs = sources_by_game.remove(&g.id).unwrap_or_default();
            let sess = sessions_by_game.remove(&g.id).unwrap_or_default();
            let parts = playtime::breakdown(
                &srcs
                    .iter()
                    .map(|s| (s.id.clone(), s.source_playtime_minutes))
                    .collect::<Vec<_>>(),
                &sess
                    .iter()
                    .map(|s| (s.game_source_id.clone(), s.seconds))
                    .collect::<Vec<_>>(),
            );
            let last_played = srcs
                .iter()
                .map(|s| s.source_last_played.clone())
                .chain(sess.iter().map(|s| s.last_ended_at.clone()))
                .fold(None, later);
            let kinds: BTreeSet<SourceKind> = srcs.iter().map(|s| s.kind).collect();
            let mut images = art.remove(&g.id).unwrap_or_default();
            GameSummary {
                installed: srcs.iter().any(|s| s.installed),
                sources: kinds.into_iter().collect(),
                playtime_minutes: playtime::total_minutes(&parts).clamp(0, u32::MAX as i64) as u32,
                last_played,
                achievements: ach
                    .get(&g.id)
                    .map(|&(done, total)| Progress { done, total }),
                goals: goal_progress.get(&g.id).copied(),
                cover: images.remove("cover"),
                header: images.remove("header"),
                id: g.id,
                title: g.title,
                sort_title: g.sort_title,
                status: g.status,
                favorite: g.favorite,
                hidden: g.hidden,
                added_at: g.created_at,
            }
        })
        .collect())
}

/// Same folding as `games::sort_title`, used for text search.
fn fold(text: &str) -> String {
    games::sort_title(text)
}

pub fn matches(game: &GameSummary, query: &LibraryQuery) -> bool {
    if game.hidden != (query.view == LibraryView::Hidden) {
        return false;
    }
    let in_view = match query.view {
        LibraryView::All | LibraryView::Hidden => true,
        LibraryView::Installed => game.installed,
        LibraryView::Favorites => game.favorite,
        LibraryView::Status(s) => game.status == s,
        LibraryView::Source(k) => game.sources.contains(&k),
    };
    if !in_view {
        return false;
    }
    match query
        .text
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        None => true,
        Some(text) => {
            let needle = fold(text);
            fold(&game.title).contains(&needle) || game.sort_title.contains(&needle)
        }
    }
}

pub fn filter_and_sort(mut list: Vec<GameSummary>, query: &LibraryQuery) -> Vec<GameSummary> {
    list.retain(|g| matches(g, query));
    let by_name = |a: &GameSummary, b: &GameSummary| a.sort_title.cmp(&b.sort_title);
    match query.sort {
        SortKey::Name => list.sort_by(by_name),
        SortKey::Playtime => list.sort_by(|a, b| {
            b.playtime_minutes
                .cmp(&a.playtime_minutes)
                .then_with(|| by_name(a, b))
        }),
        SortKey::Added => {
            list.sort_by(|a, b| b.added_at.cmp(&a.added_at).then_with(|| by_name(a, b)))
        }
        // Most recent first, never-played games last.
        SortKey::LastPlayed => list.sort_by(|a, b| match (&a.last_played, &b.last_played) {
            (Some(x), Some(y)) => y.cmp(x).then_with(|| by_name(a, b)),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => by_name(a, b),
        }),
    }
    list
}

pub fn counts(list: &[GameSummary]) -> LibraryCounts {
    let mut c = LibraryCounts::default();
    let mut by_source: HashMap<SourceKind, u32> = HashMap::new();
    c.hidden = list.iter().filter(|g| g.hidden).count() as u32;
    for g in list.iter().filter(|g| !g.hidden) {
        c.all += 1;
        c.installed += g.installed as u32;
        c.favorites += g.favorite as u32;
        match g.status {
            GameStatus::Todo => c.status.todo += 1,
            GameStatus::Playing => c.status.playing += 1,
            GameStatus::Done => c.status.done += 1,
            GameStatus::Dropped => c.status.dropped += 1,
        }
        for k in &g.sources {
            *by_source.entry(*k).or_default() += 1;
        }
    }
    c.sources = SourceKind::ALL
        .iter()
        .filter_map(|k| {
            by_source
                .get(k)
                .map(|&count| SourceCount { kind: *k, count })
        })
        .collect();
    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repo::game_sources::SourceData;
    use crate::db::Db;

    fn game(title: &str) -> GameSummary {
        GameSummary {
            id: title.to_string(),
            title: title.to_string(),
            sort_title: games::sort_title(title),
            status: GameStatus::Todo,
            favorite: false,
            hidden: false,
            installed: false,
            sources: vec![SourceKind::Steam],
            playtime_minutes: 0,
            last_played: None,
            added_at: "2026-01-01T00:00:00Z".into(),
            cover: None,
            header: None,
            achievements: None,
            goals: None,
        }
    }

    fn q(view: LibraryView, text: Option<&str>, sort: SortKey) -> LibraryQuery {
        LibraryQuery {
            view,
            text: text.map(Into::into),
            sort,
        }
    }

    fn titles(list: &[GameSummary]) -> Vec<&str> {
        list.iter().map(|g| g.title.as_str()).collect()
    }

    fn sample() -> Vec<GameSummary> {
        let mut a = game("The Witcher 3");
        a.installed = true;
        a.playtime_minutes = 500;
        a.last_played = Some("2026-03-01T10:00:00Z".into());
        let mut b = game("Élden Ring");
        b.favorite = true;
        b.status = GameStatus::Playing;
        b.playtime_minutes = 900;
        b.last_played = Some("2026-05-01T10:00:00Z".into());
        b.added_at = "2026-02-01T00:00:00Z".into();
        let mut c = game("Alan Wake 2");
        c.sources = vec![SourceKind::Epic];
        let mut hidden = game("Hidden demo");
        hidden.hidden = true;
        vec![a, b, c, hidden]
    }

    #[test]
    fn views_filter_and_hidden_is_excluded() {
        let all = sample();
        assert_eq!(
            filter_and_sort(all.clone(), &q(LibraryView::All, None, SortKey::Name)).len(),
            3
        );
        assert_eq!(
            titles(&filter_and_sort(
                all.clone(),
                &q(LibraryView::Installed, None, SortKey::Name)
            )),
            ["The Witcher 3"]
        );
        assert_eq!(
            titles(&filter_and_sort(
                all.clone(),
                &q(LibraryView::Favorites, None, SortKey::Name)
            )),
            ["Élden Ring"]
        );
        assert_eq!(
            titles(&filter_and_sort(
                all.clone(),
                &q(
                    LibraryView::Status(GameStatus::Playing),
                    None,
                    SortKey::Name
                )
            )),
            ["Élden Ring"]
        );
        assert_eq!(
            titles(&filter_and_sort(
                all,
                &q(LibraryView::Source(SourceKind::Epic), None, SortKey::Name)
            )),
            ["Alan Wake 2"]
        );
    }

    #[test]
    fn hidden_view_lists_only_hidden_games() {
        let list = filter_and_sort(sample(), &q(LibraryView::Hidden, None, SortKey::Name));
        assert_eq!(titles(&list), ["Hidden demo"]);
    }

    #[test]
    fn text_search_ignores_case_accents_and_articles() {
        let all = sample();
        let find = |t: &str| {
            titles(&filter_and_sort(
                all.clone(),
                &q(LibraryView::All, Some(t), SortKey::Name),
            ))
            .len()
        };
        assert_eq!(find("elden"), 1);
        assert_eq!(find("WITCHER"), 1);
        assert_eq!(find("the witcher"), 1);
        assert_eq!(find("  "), 3);
        assert_eq!(find("zelda"), 0);
    }

    #[test]
    fn sorts() {
        let all = sample();
        let sorted =
            |s| titles(&filter_and_sort(all.clone(), &q(LibraryView::All, None, s))).join(", ");
        assert_eq!(
            sorted(SortKey::Name),
            "Alan Wake 2, Élden Ring, The Witcher 3"
        );
        assert_eq!(
            sorted(SortKey::Playtime),
            "Élden Ring, The Witcher 3, Alan Wake 2"
        );
        assert_eq!(
            sorted(SortKey::LastPlayed),
            "Élden Ring, The Witcher 3, Alan Wake 2"
        );
        assert_eq!(
            sorted(SortKey::Added),
            "Élden Ring, Alan Wake 2, The Witcher 3"
        );
    }

    #[test]
    fn counts_skip_hidden_games() {
        let c = counts(&sample());
        assert_eq!(c.all, 3);
        assert_eq!(c.installed, 1);
        assert_eq!(c.favorites, 1);
        assert_eq!(c.hidden, 1);
        assert_eq!(
            c.status,
            StatusCounts {
                todo: 2,
                playing: 1,
                done: 0,
                dropped: 0
            }
        );
        assert_eq!(
            c.sources,
            vec![
                SourceCount {
                    kind: SourceKind::Steam,
                    count: 2
                },
                SourceCount {
                    kind: SourceKind::Epic,
                    count: 1
                }
            ]
        );
    }

    #[test]
    fn summaries_apply_the_cumul_rule() {
        let db = Db::open_in_memory().unwrap();
        let conn = db.conn();
        let g = games::insert(&conn, "Hades").unwrap();
        let steam = game_sources::upsert(
            &conn,
            &g.id,
            SourceKind::Steam,
            &SourceData {
                external_id: Some("1145360".into()),
                installed: true,
                source_playtime_minutes: Some(120),
                source_last_played: Some("2026-01-01T00:00:00Z".into()),
                ..Default::default()
            },
        )
        .unwrap();
        conn.execute(
            "INSERT INTO play_sessions (id, game_id, game_source_id, started_at, ended_at, duration_seconds, origin) \
             VALUES ('s1', ?1, ?2, '2026-02-01T00:00:00Z', '2026-02-01T01:00:00Z', 3600, 'auto'), \
                    ('s2', ?1, NULL, NULL, NULL, 1800, 'estimate')",
            rusqlite::params![g.id, steam.id],
        )
        .unwrap();
        let list = summaries(&conn).unwrap();
        assert_eq!(list.len(), 1);
        // 120 (Steam counter, its auto session not added) + 30 (estimate without source)
        assert_eq!(list[0].playtime_minutes, 150);
        assert_eq!(list[0].last_played.as_deref(), Some("2026-02-01T01:00:00Z"));
        assert!(list[0].installed);
        assert_eq!(list[0].sources, vec![SourceKind::Steam]);
    }
}
