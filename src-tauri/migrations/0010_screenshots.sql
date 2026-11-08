-- Captures tab (docs/FICHE-ONGLETS.md). Only metadata: the files stay where
-- Steam, the Game Bar or the user put them (read-only). The session of a
-- capture is computed when listing (sessions can be edited or merged).
CREATE TABLE screenshots (
    id          TEXT PRIMARY KEY,
    game_id     TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    source      TEXT NOT NULL CHECK (source IN ('steam', 'gamebar', 'folder')),
    path        TEXT NOT NULL UNIQUE,
    kind        TEXT NOT NULL DEFAULT 'image' CHECK (kind IN ('image', 'video')),
    taken_at    TEXT NOT NULL,
    width       INTEGER,
    height      INTEGER,
    bytes       INTEGER NOT NULL DEFAULT 0,
    caption     TEXT,
    thumb_path  TEXT
);
CREATE INDEX screenshots_game ON screenshots(game_id, taken_at);

-- Folders added by the user; game_id NULL = "all games" (matched by name).
CREATE TABLE screenshot_folders (
    path     TEXT PRIMARY KEY,
    game_id  TEXT REFERENCES games(id) ON DELETE CASCADE
);
