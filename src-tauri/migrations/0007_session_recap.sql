-- End-of-session recap (docs/SESSION-RECAP.md): session rating, resume note,
-- and the idle stretches removed from a session (for its timeline).
ALTER TABLE play_sessions ADD COLUMN rating INTEGER CHECK (rating BETWEEN 1 AND 5);
ALTER TABLE games ADD COLUMN resume_note TEXT;
ALTER TABLE games ADD COLUMN resume_note_at TEXT;
CREATE TABLE session_idle_periods (
  session_id TEXT NOT NULL REFERENCES play_sessions(id) ON DELETE CASCADE,
  started_at TEXT NOT NULL,
  ended_at TEXT NOT NULL
);
CREATE INDEX idx_idle_session ON session_idle_periods(session_id);
