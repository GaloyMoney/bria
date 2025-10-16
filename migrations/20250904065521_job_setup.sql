CREATE TABLE jobs (
  id UUID PRIMARY KEY,
  unique_per_type BOOLEAN NOT NULL,
  job_type VARCHAR NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE UNIQUE INDEX idx_unique_job_type ON jobs (job_type) WHERE unique_per_type = TRUE;

CREATE TABLE job_events (
  id UUID NOT NULL REFERENCES jobs(id),
  sequence INT NOT NULL,
  event_type VARCHAR NOT NULL,
  event JSONB NOT NULL,
  context JSONB DEFAULT NULL,
  recorded_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE(id, sequence)
);

CREATE TYPE JobExecutionState AS ENUM ('pending', 'running');

CREATE TABLE job_executions (
  id UUID REFERENCES jobs(id) NOT NULL UNIQUE,
  job_type VARCHAR NOT NULL,
  poller_instance_id UUID,
  attempt_index INT NOT NULL DEFAULT 1,
  state JobExecutionState NOT NULL DEFAULT 'pending',
  execution_state_json JSONB,
  execute_at TIMESTAMPTZ,
  alive_at TIMESTAMPTZ NOT NULL,
  created_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_job_executions_poller_instance
  ON job_executions(poller_instance_id)
  WHERE state = 'running';

CREATE OR REPLACE FUNCTION notify_job_execution_insert() RETURNS TRIGGER AS $$
BEGIN
  PERFORM pg_notify('job_execution', '');
  RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION notify_job_execution_update() RETURNS TRIGGER AS $$
BEGIN
  IF NEW.execute_at IS DISTINCT FROM OLD.execute_at THEN
    PERFORM pg_notify('job_execution', '');
  END IF;
  RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER job_executions_notify_insert_trigger
AFTER INSERT ON job_executions
FOR EACH STATEMENT
EXECUTE FUNCTION notify_job_execution_insert();

CREATE TRIGGER job_executions_notify_update_trigger
AFTER UPDATE ON job_executions
FOR EACH STATEMENT
EXECUTE FUNCTION notify_job_execution_update();
