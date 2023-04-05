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

@test "lnd_sync: Incoming tx" {
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
  [[ $(cached_pending_income) == 100000000 ]] || exit 1

  utxos=$(bria_cmd list-utxos -w default)
  n_utxos=$(jq '.keychains[0].utxos | length' <<< "${utxos}")
  utxo_block_height=$(jq -r '.keychains[0].utxos[0].blockHeight' <<< "${utxos}")

  [[ "${n_utxos}" == "1" && "${utxo_block_height}" == "null" ]]

  bitcoin_cli -generate 2

  for i in {1..30}; do
    cache_default_wallet_balance
    [[ $(cached_current_settled) == 100000000 ]] && break
    sleep 1
  done
  [[ $(cached_current_settled) == 100000000 ]] || exit 1;

  utxos=$(bria_cmd list-utxos -w default)
  n_utxos=$(jq '.keychains[0].utxos | length' <<< "${utxos}")
  utxo_block_height=$(jq -r '.keychains[0].utxos[0].blockHeight' <<< "${utxos}")

  [[ "${n_utxos}" == "1" && "${utxo_block_height}" == "201" ]]
}

@test "lnd_sync: Detects outgoing transactions" {
  # Get an address from bitcoind (or use a hardcoded regtest address)
  bitcoind_address=$(bitcoin_cli -regtest getnewaddress)

  # Send funds from LND to the bitcoind address
  lnd_cli sendcoins --addr=${bitcoind_address} --amt=50000000

  # Wait for Bria to detect the outgoing transaction
  for i in {1..30}; do
    cache_default_wallet_balance
    [[ $(cached_pending_outgoing) == 50000000 ]] && break
    sleep 1
  done
  [[ $(cached_pending_outgoing) == 50000000 ]] || exit 1
  [[ $(cached_current_settled) == 0 ]] || exit 1

  utxos=$(bria_cmd list-utxos -w default)
  n_utxos=$(jq '.keychains[0].utxos | length' <<< "${utxos}")
  change=$(jq -r '.keychains[0].utxos[0].changeOutput' <<< "${utxos}")

  [[ "${n_utxos}" == "1" && "${change}" == "true" ]]

  # Generate a block to confirm the transaction
  bitcoin_cli -generate 1

  # Wait for Bria to detect the confirmed outgoing transaction
  for i in {1..30}; do
    cache_default_wallet_balance
    [[ $(cached_current_settled) != 0 ]] && break
    sleep 1
  done
  [[ $(cached_pending_outgoing) == 0 ]] || exit 1

  utxos=$(bria_cmd list-utxos -w default)
  n_utxos=$(jq '.keychains[0].utxos | length' <<< "${utxos}")
  utxo_block_height=$(jq -r '.keychains[0].utxos[0].blockHeight' <<< "${utxos}")

  [[ "${n_utxos}" == "1" && "${utxo_block_height}" == "203" ]]
}
