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

@test "payjoin: Start payjoin session and retrieve uri" {
  bria_address=$(bria_cmd new-address -w default | jq -r '.address')
  if [ -z "$bria_address" ]; then
    echo "Failed to get a new address"
    exit 1
  fi

  bitcoin_cli -rpcwallet=default -regtest sendtoaddress ${bria_address} 1

  for i in {1..30}; do
   n_utxos=$(bria_cmd list-utxos -w default | jq '.keychains[0].utxos | length')
    [[ "${n_utxos}" == "3" ]] && break
    sleep 1
  done
  cache_wallet_balance
  [[ $(cached_encumbered_fees) != 0 ]] || exit 1
  [[ $(cached_pending_income) == 100000000 ]] || exit 1;

  bria_uri=$(bria_cmd new-uri -w default | jq -r '.uri')
  if [ -z "$bria_uri" ] || [ "$bria_uri" = "null" ]; then
    echo "Failed to get a new uri"
    exit 1
  fi
  echo $bria_uri
  # payjoin_cli send --fee-rate 2 ${bria_uri}

  # for i in {1..30}; do
  #  n_utxos=$(bria_cmd list-utxos -w default | jq '.keychains[0].utxos | length')
  #   [[ "${n_utxos}" == "3" ]] && break
  #   sleep 1
  # done
  # cache_wallet_balance
  # [[ $(cached_encumbered_fees) != 0 ]] || exit 1
  # [[ $(cached_pending_income) == 220000000 ]] || exit 1;
}
