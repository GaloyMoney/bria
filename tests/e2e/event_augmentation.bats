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

@test "event_augmentation: adds up to date address info to events" {
  bria_address=$(bria_cmd new-address -w default -m '{"hello":"world"}' | jq -r '.address')
  bitcoin_cli -regtest sendtoaddress ${bria_address} 1
  event=$(bria_cmd watch-events -a 0 -o | jq -r '.augmentation')
  [ "$event" = "null" ] || exit 1
  meta=$(bria_cmd watch-events -a 0 -o --augment | jq -r '.augmentation.addressInfo.metadata.hello')
  [ "$meta" = "world" ] || exit 1
  bria_cmd update-address -a "${bria_address}" -m '{"other":"world"}'
  meta=$(bria_cmd watch-events -a 0 -o --augment | jq -r '.augmentation.addressInfo.metadata.other')
  [ "$meta" = "world" ] || exit 1
}
