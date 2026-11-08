-- Achievements tab (docs/FICHE-ONGLETS.md): Steam hidden achievements keep
-- their description masked until unlocked or revealed.
ALTER TABLE achievements ADD COLUMN hidden INTEGER NOT NULL DEFAULT 0;
