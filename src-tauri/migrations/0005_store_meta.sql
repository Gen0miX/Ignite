-- When genres / player modes were last fetched from the platform store.
ALTER TABLE game_sources ADD COLUMN store_meta_synced_at TEXT;
