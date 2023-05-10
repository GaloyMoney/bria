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

@test "event_augmentation: adds payout info to events" {
  bria_cmd create-payout-queue --name high --interval-trigger 5
  bria_cmd submit-payout --wallet default --queue-name high --destination bcrt1q208tuy5rd3kvy8xdpv6yrczg7f3mnlk3lql7ej --amount 75000000 -e "external"
  external_id=$(bria_cmd watch-events -a 1 -o --augment | jq -r '.augmentation.payoutInfo.externalId')
  [ "$external_id" = "external" ] || exit 1
}
