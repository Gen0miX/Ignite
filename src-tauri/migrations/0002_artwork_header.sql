-- Allow the 'header' artwork kind (Steam 460x215 capsule, used when no 600x900 cover exists).
CREATE TABLE artwork_new (
  game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
  kind TEXT NOT NULL CHECK (kind IN ('cover', 'hero', 'logo', 'icon', 'header')),
  local_path TEXT NOT NULL,
  provider TEXT,
  source_url TEXT,
  PRIMARY KEY (game_id, kind)
);
INSERT INTO artwork_new SELECT game_id, kind, local_path, provider, source_url FROM artwork;
DROP TABLE artwork;
ALTER TABLE artwork_new RENAME TO artwork;
