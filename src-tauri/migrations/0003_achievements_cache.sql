-- Achievement cache bookkeeping per source: when it was fetched and how many
-- achievements the platform reports (0 = the game has none, NULL = unknown).
ALTER TABLE game_sources ADD COLUMN achievements_synced_at TEXT;
ALTER TABLE game_sources ADD COLUMN achievements_total INTEGER;
CREATE INDEX idx_goals_game ON goals(game_id, position);
CREATE INDEX idx_achievements_source ON achievements(game_source_id);
