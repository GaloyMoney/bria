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

@test "outbox: utxo_dropped event" {
  bria_address=$(bria_cmd new-address -w default | jq -r '.address')
  bitcoin_cli -regtest sendtoaddress ${bria_address} 1
  for i in {1..30}; do
    n_utxos=$(bria_cmd list-utxos -w default | jq '.keychains[0].utxos | length')
    [[ "${n_utxos}" == "1" ]] && break
    sleep 1
  done
  event=$(bria_cmd watch-events -a 0 -o | jq -r '.payload.utxoDetected')
  [ "$event" != "null" ] || exit 1
  cache_default_wallet_balance
  [[ $(cached_pending_income) == 100000000 ]] || exit 1;

  restart_bitcoin_stack
  bitcoind_init

  event=$(bria_cmd watch-events -a 1 -o | jq -r '.payload.utxoDropped')
  [ "$event" != "null" ] || exit 1

  cache_default_wallet_balance
  [[ $(cached_pending_income) == 0 ]] || exit 1;
}

@test "outbox: adds address augmentation to events" {
  bria_address=$(bria_cmd new-address -w default -m '{"hello":"world"}' | jq -r '.address')
  bitcoin_cli -regtest sendtoaddress ${bria_address} 1
  event=$(bria_cmd watch-events -a 2 -o | jq -r '.augmentation')
  [ "$event" = "null" ] || exit 1
  meta=$(bria_cmd watch-events -a 2 -o --augment | jq -r '.augmentation.addressInfo.metadata.hello')
  [ "$meta" = "world" ] || exit 1
  bria_cmd update-address -a "${bria_address}" -m '{"other":"world"}'
  meta=$(bria_cmd watch-events -a 2 -o --augment | jq -r '.augmentation.addressInfo.metadata.other')
  [ "$meta" = "world" ] || exit 1
}

@test "outbox: adds payout augmentation info to events" {
  bria_cmd create-payout-queue --name high --interval-trigger 5
  bria_cmd submit-payout --wallet default --queue-name high --destination bcrt1q208tuy5rd3kvy8xdpv6yrczg7f3mnlk3lql7ej --amount 75000000 -e "external"
  external_id=$(bria_cmd watch-events -a 3 -o --augment | jq -r '.augmentation.payoutInfo.externalId')
  [ "$external_id" = "external" ] || exit 1
}
