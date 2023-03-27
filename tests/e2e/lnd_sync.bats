#!/usr/bin/env bats

load "helpers"

setup_file() {
  restart_bitcoin
  reset_pg
  bitcoind_init
  start_daemon
  bria_init
}

teardown_file() {
  stop_daemon
}

@test "lnd_sync: Generates the same address" {
  lnd_address=$(lnd_cli newaddress p2wkh | jq -r '.address')
  bria_address=$(bria_cmd new-address -w default | jq -r '.address')

  [ "$lnd_address" = "$bria_address" ]
}

@test "lnd_sync: Mirrors balance" {
  lnd_address=$(lnd_cli newaddress p2wkh | jq -r '.address')
  if [ -z "$lnd_address" ]; then
    echo "Failed to get a new address"
    exit 1
  fi

  bitcoin_cli -regtest sendtoaddress ${lnd_address} 1

  for i in {1..30}; do
    cache_default_wallet_balance
    [[ $(cached_pending_income) == 100000000 ]] && break
    sleep 1
  done
  [[ $(cached_pending_income) == 100000000 ]] || exit 1;

  bitcoin_cli -generate 6

  for i in {1..30}; do
    cache_default_wallet_balance
    [[ $(cached_current_settled) == 100000000 ]] && break
    sleep 1
  done
  [[ $(cached_current_settled) == 100000000 ]] || exit 1;
}

