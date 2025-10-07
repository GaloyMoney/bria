#!/usr/bin/env bats

load "helpers"

setup_file() {
  restart_bitcoin_stack
  reset_pg
  bitcoind_init
  start_daemon
  bria_init
}

teardown_file() {
  stop_daemon
}

@test "dummy_job: Verify job exists in database" {
  # Wait a few seconds for daemon to initialize
  sleep 10
  
  # Query the jobs table to check if dummy job exists
  job_count=$(${DOCKER_ENGINE} exec "${COMPOSE_PROJECT_NAME}-postgres-1" psql $PG_CON -t -c "SELECT COUNT(*) FROM jobs WHERE job_type = 'dummy';")
  
  # Trim whitespace
  job_count=$(echo $job_count | xargs)
  
  if [[ $job_count -lt 1 ]]; then
    echo "Dummy job not found in database"
    echo "Available jobs:"
    ${DOCKER_ENGINE} exec "${COMPOSE_PROJECT_NAME}-postgres-1" psql $PG_CON -c "SELECT job_type, created_at FROM jobs;"
    exit 1
  fi
  
  echo "Found $job_count dummy job(s) in database"
}

@test "dummy_job: Verify job has execution scheduled" {
  # Check if there are any scheduled executions for the dummy job
  exec_count=$(${DOCKER_ENGINE} exec "${COMPOSE_PROJECT_NAME}-postgres-1" psql $PG_CON -t -c "SELECT COUNT(*) FROM job_executions je JOIN jobs j ON je.id = j.id WHERE j.job_type = 'dummy';")
  
  # Trim whitespace  
  exec_count=$(echo $exec_count | xargs)
  
  if [[ $exec_count -lt 1 ]]; then
    echo "No executions scheduled for dummy job"
    exit 1
  fi
  
  echo "Found $exec_count scheduled execution(s) for dummy job"
}

