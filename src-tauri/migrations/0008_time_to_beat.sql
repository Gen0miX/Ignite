-- Time to beat (docs/TIME-TO-BEAT.md): one row per game and provider. The
-- user's correction ('manual') always wins over IGDB; games.hltb_main_minutes
-- keeps the best "main story" value for the library and smart collections.
CREATE TABLE game_time_to_beat (
  game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
  provider TEXT NOT NULL CHECK (provider IN ('manual', 'igdb')),
  external_id TEXT,
  main_minutes INTEGER,
  extra_minutes INTEGER,
  completionist_minutes INTEGER,
  sample_count INTEGER,
  url TEXT,
  fetched_at TEXT NOT NULL,
  PRIMARY KEY (game_id, provider)
);
