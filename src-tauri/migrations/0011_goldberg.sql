-- Steam appid used by Goldberg for a game added by hand (local executable).
-- NULL = not configured yet.
ALTER TABLE game_sources ADD COLUMN goldberg_appid INTEGER;
