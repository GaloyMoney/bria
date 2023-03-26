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
  # Get a p2wkh address from lnd
  lnd_address=$(lnd_cli newaddress p2wkh | jq -r '.address')

  # Get a new address from bria
  bria_address=$(bria_cmd new-address -w default --raw)

  # Assert they are equal
  echo $lnd_address
  echo $bria_address
  [ "$lnd_address" = "$bria_address" ]
}

@test "lnd_sync: Mirrors balance" {
  # Create a new onchain address using the lnd_cli helper
  lnd_address=$(lnd_cli newaddress p2wkh | jq -r '.address')
  if [ -z "$lnd_address" ]; then
    echo "Failed to get a new address"
    exit 1
  fi

  # Send money to the new address
  bitcoin_cli -regtest sendtoaddress ${lnd_address} 1

  # Check via bria_cmd that it is being observed
  for i in {1..30}; do
    cache_default_wallet_balance
    [[ $(cached_pending_income) == 100000000 ]] && break
    sleep 1
  done
  [[ $(cached_pending_income) == 100000000 ]] || exit 1;

  # Create some blocks
  bitcoin_cli -generate 6

  # Check that the balance has confirmed in bria
  for i in {1..30}; do
    cache_default_wallet_balance
    [[ $(cached_current_settled) == 100000000 ]] && break
    sleep 1
  done
  [[ $(cached_current_settled) == 100000000 ]] || exit 1;
}

