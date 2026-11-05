-- RetroAchievements game id of an emulated ROM (found by hash or set by hand).
ALTER TABLE game_sources ADD COLUMN ra_game_id INTEGER;
