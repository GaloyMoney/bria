ALTER TABLE bria_signing_sessions ADD COLUMN created_at TIMESTAMPTZ;

UPDATE bria_signing_sessions
SET created_at =(
  SELECT MIN(recorded_at)
  FROM bria_signing_session_events
  WHERE bria_signing_session_events.id = bria_signing_sessions.id
);

ALTER TABLE bria_signing_sessions
ALTER COLUMN created_at SET NOT NULL,
ALTER COLUMN created_at SET DEFAULT NOW();
