#!/usr/bin/env bats

load "helpers"

setup_file() {
  restart_bitcoin
  reset_pg
  bitcoind_init
  start_daemon
  bria_lnd_init
}

teardown_file() {
  stop_daemon
}

@test "lnd_sync: Generates the same address" {
  lnd_address=$(lnd_cli newaddress p2wkh | jq -r '.address')
  bria_address=$(bria_cmd new-address -w default | jq -r '.address')

  [ "$lnd_address" = "$bria_address" ]

  n_addresses=$(bria_cmd list-addresses -w default | jq -r '.addresses | length')
  [ "$n_addresses" = "1" ] || exit 1
}

@test "lnd_sync: Detects incoming transactions" {
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

  n_addresses=$(bria_cmd list-addresses -w default | jq -r '.addresses | length')
  [ "$n_addresses" = "2" ] || exit 1
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
  bitcoind_address=$(bitcoin_cli -regtest getnewaddress)
  lnd_cli sendcoins --addr=${bitcoind_address} --amt=50000000
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

  bitcoin_cli -generate 1

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

@test "lnd_sync: Can handle spend from mix of unconfirmed UTXOs" {
  lnd_address=$(lnd_cli newaddress p2wkh | jq -r '.address')
  if [ -z "$lnd_address" ]; then
    echo "Failed to get a new address"
    exit 1
  fi

  bitcoin_cli -regtest sendtoaddress ${lnd_address} 1
  bitcoin_cli -regtest sendtoaddress ${lnd_address} 1

  bitcoind_address=$(bitcoin_cli -regtest getnewaddress)
  lnd_cli sendcoins --addr=${bitcoind_address} --amt=210000000 --min_confs 0

  for i in {1..30}; do
    cache_default_wallet_balance
    [[ $(cached_pending_outgoing) == 210000000 ]] && break
    sleep 1
  done
  [[ $(cached_pending_outgoing) == 210000000 ]] || exit 1
  [[ $(cached_logical_settled) != 0 ]] || exit 1

  bitcoin_cli -generate 2
  for i in {1..30}; do
    cache_default_wallet_balance
    [[ $(cached_pending_outgoing) == 0 ]] && break
    sleep 1
  done

  lnd_balance=$(lnd_cli walletbalance | jq -r '.total_balance')
  [[ "$(cached_logical_settled)" == "${lnd_balance}" ]] || exit 1
}

@test "lnd_sync: Can sweep all" {
  bitcoind_address=$(bitcoin_cli -regtest getnewaddress)
  lnd_cli sendcoins --addr=${bitcoind_address} --sweepall
  bitcoin_cli -generate 1

  for i in {1..30}; do
    cache_default_wallet_balance
    [[ $(cached_encumbered_fees) == 0 ]] && break
    sleep 1
  done
  [[ $(cached_encumbered_fees) == 0 ]] || exit 1
  [[ $(cached_logical_settled) == 0 ]] || exit 1
}

@test "lnd_sync: Can spend only from unconfirmed" {
  lnd_address=$(lnd_cli newaddress p2wkh | jq -r '.address')
  bitcoin_cli -regtest sendtoaddress ${lnd_address} 1
  bitcoind_address=$(bitcoin_cli -regtest getnewaddress)
  lnd_cli sendcoins --addr=${bitcoind_address} --amt=60000000 --min_confs 0

  for i in {1..30}; do
    cache_default_wallet_balance
    [[ $(cached_pending_outgoing) == 60000000 ]] && break
    sleep 1
  done
  [[ $(cached_pending_outgoing) == 60000000 ]] || exit 1
  [[ $(cached_logical_settled) == 0 ]] || exit 1

  bitcoin_cli -generate 2
  for i in {1..30}; do
    cache_default_wallet_balance
    [[ $(cached_pending_outgoing) == 0 ]] && break
    sleep 1
  done
  [[ $(cached_pending_outgoing) == 0 ]] || exit 1
  [[ $(cached_logical_settled) == $(cached_current_settled) ]] || exit 1
  lnd_balance=$(lnd_cli walletbalance | jq -r '.total_balance')
  [[ "$(cached_logical_settled)" == "${lnd_balance}" ]] || exit 1
}
