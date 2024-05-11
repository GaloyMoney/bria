#!/usr/bin/env bats
RUST_LOG=debug

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

@test "setup" {
  echo "done"
}
