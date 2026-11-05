-- Daily snapshot of platform playtime counters (Steam, GOG): the difference
-- between two snapshots is the time played meanwhile (stats per period).
CREATE TABLE playtime_snapshots (
  game_source_id TEXT NOT NULL REFERENCES game_sources(id) ON DELETE CASCADE,
  day TEXT NOT NULL,          -- YYYY-MM-DD, local date
  minutes INTEGER NOT NULL,
  PRIMARY KEY (game_source_id, day)
);

-- When the game was marked "done" (games finished per period).
ALTER TABLE games ADD COLUMN completed_at TEXT;
UPDATE games SET completed_at = updated_at WHERE status = 'done';
