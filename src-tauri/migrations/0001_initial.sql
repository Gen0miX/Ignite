-- Initial schema (docs/ARCHITECTURE.md § 3).
-- Deviations from the doc: foreign keys added on game_tags / collection_games,
-- index on game_sources(game_id).

CREATE TABLE games (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  sort_title TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'todo' CHECK (status IN ('todo', 'playing', 'done', 'dropped')),
  favorite INTEGER NOT NULL DEFAULT 0,
  hidden INTEGER NOT NULL DEFAULT 0,
  notes TEXT,
  preferred_source_id TEXT,
  igdb_id INTEGER,
  steamgriddb_id INTEGER,
  hltb_main_minutes INTEGER,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE INDEX idx_games_sort_title ON games(sort_title);

CREATE TABLE game_sources (
  id TEXT PRIMARY KEY,
  game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
  kind TEXT NOT NULL,
  external_id TEXT,
  installed INTEGER NOT NULL DEFAULT 0,
  install_path TEXT,
  exe_path TEXT,
  size_bytes INTEGER,
  launch_uri TEXT,
  process_matchers TEXT,
  source_playtime_minutes INTEGER,
  source_last_played TEXT,
  last_synced_at TEXT,
  UNIQUE (kind, external_id)
);
CREATE INDEX idx_game_sources_game ON game_sources(game_id);

CREATE TABLE launch_profiles (
  id TEXT PRIMARY KEY,
  game_source_id TEXT NOT NULL REFERENCES game_sources(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  is_default INTEGER NOT NULL DEFAULT 0,
  mode TEXT NOT NULL DEFAULT 'auto' CHECK (mode IN ('auto', 'uri', 'exe')),
  args TEXT,
  env TEXT,
  working_dir TEXT,
  pre_command TEXT,
  post_command TEXT
);

CREATE TABLE play_sessions (
  id TEXT PRIMARY KEY,
  game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
  game_source_id TEXT REFERENCES game_sources(id) ON DELETE SET NULL,
  started_at TEXT,
  ended_at TEXT,
  duration_seconds INTEGER NOT NULL,
  idle_seconds INTEGER NOT NULL DEFAULT 0,
  origin TEXT NOT NULL CHECK (origin IN ('auto', 'manual', 'imported', 'estimate')),
  note TEXT
);
CREATE INDEX idx_sessions_game ON play_sessions(game_id, started_at);

CREATE TABLE achievements (
  id TEXT PRIMARY KEY,
  game_source_id TEXT NOT NULL REFERENCES game_sources(id) ON DELETE CASCADE,
  api_name TEXT NOT NULL,
  name TEXT,
  description TEXT,
  icon_url TEXT,
  unlocked_at TEXT,
  global_percent REAL,
  UNIQUE (game_source_id, api_name)
);

CREATE TABLE goals (
  id TEXT PRIMARY KEY,
  game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
  title TEXT NOT NULL,
  position INTEGER NOT NULL,
  done_at TEXT
);

CREATE TABLE tags (
  id TEXT PRIMARY KEY,
  name TEXT UNIQUE NOT NULL,
  origin TEXT
);

CREATE TABLE game_tags (
  game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
  tag_id TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
  PRIMARY KEY (game_id, tag_id)
);

CREATE TABLE collections (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('manual', 'smart')),
  rules TEXT,
  position INTEGER NOT NULL
);

CREATE TABLE collection_games (
  collection_id TEXT NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
  game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
  PRIMARY KEY (collection_id, game_id)
);

CREATE TABLE artwork (
  game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
  kind TEXT NOT NULL CHECK (kind IN ('cover', 'hero', 'logo', 'icon')),
  local_path TEXT NOT NULL,
  provider TEXT,
  source_url TEXT,
  PRIMARY KEY (game_id, kind)
);

CREATE TABLE settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
