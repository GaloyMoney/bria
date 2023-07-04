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

@test "bitcoind_signer_sync: Generates the same address" {
  bitcoind_signer_address=$(bitcoin_signer_cli getnewaddress)
  bria_address=$(bria_cmd new-address -w default | jq -r '.address')

  [ "$bitcoind_signer_address" = "$bria_address" ] || exit 1

  n_addresses=$(bria_cmd list-addresses -w default | jq -r '.addresses | length')
  [ "$n_addresses" = "1" ] || exit 1
}

@test "bitcoind_signer_sync: Detects incoming transactions" {
  bitcoind_signer_address=$(bitcoin_signer_cli getnewaddress)
  if [ -z "$bitcoind_signer_address" ]; then
    echo "Failed to get a new address"
    exit 1
  fi

  bitcoin_cli -regtest sendtoaddress ${bitcoind_signer_address} 1

  for i in {1..30}; do
    cache_wallet_balance
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
    cache_wallet_balance
    [[ $(cached_current_settled) == 100000000 ]] && break
    sleep 1
  done
  [[ $(cached_current_settled) == 100000000 ]] || exit 1;

  utxos=$(bria_cmd list-utxos -w default)
  n_utxos=$(jq '.keychains[0].utxos | length' <<< "${utxos}")
  utxo_block_height=$(jq -r '.keychains[0].utxos[0].blockHeight' <<< "${utxos}")

  [[ "${n_utxos}" == "1" && "${utxo_block_height}" == "201" ]] || exit 1
}

@test "bitcoind_signer_sync: Detects outgoing transactions" {
  bitcoind_address=$(bitcoin_cli -regtest getnewaddress)
  bitcoin_signer_cli -regtest sendtoaddress "${bitcoind_address}" 0.5
  for i in {1..30}; do
    cache_wallet_balance
    [[ $(cached_pending_outgoing) == 50000000 ]] && break
    sleep 1
  done
  [[ $(cached_pending_outgoing) == 50000000 ]] || exit 1
  [[ $(cached_current_settled) == 0 ]] || exit 1

  utxos=$(bria_cmd list-utxos -w default)
  n_utxos=$(jq '.keychains[0].utxos | length' <<< "${utxos}")
  change=$(jq -r '.keychains[0].utxos[0].changeOutput' <<< "${utxos}")

  [[ "${n_utxos}" == "1" && "${change}" == "true" ]] || exit 1

  bitcoin_cli -generate 1

  for i in {1..30}; do
    cache_wallet_balance
    [[ $(cached_current_settled) != 0 ]] && break
    sleep 1
  done
  [[ $(cached_pending_outgoing) == 0 ]] || exit 1

  utxos=$(bria_cmd list-utxos -w default)
  n_utxos=$(jq '.keychains[0].utxos | length' <<< "${utxos}")
  utxo_block_height=$(jq -r '.keychains[0].utxos[0].blockHeight' <<< "${utxos}")

  [[ "${n_utxos}" == "1" && "${utxo_block_height}" == "203" ]] || exit 1
}

@test "bitcoind_signer_sync: Can handle spend from mix of unconfirmed UTXOs" {
  bitcoind_signer_address=$(bitcoin_signer_cli getnewaddress)
  if [ -z "$bitcoind_signer_address" ]; then
    echo "Failed to get a new address"
    exit 1
  fi

  bitcoin_cli -regtest sendtoaddress ${bitcoind_signer_address} 1
  bitcoin_cli -regtest sendtoaddress ${bitcoind_signer_address} 1

  bitcoind_address=$(bitcoin_cli -regtest getnewaddress)
  for i in {1..20}; do
    [[ $(bitcoin_signer_cli getunconfirmedbalance) == "2.00000000" ]] && break
    sleep 1
  done

  bitcoin_signer_cli_send_all_utxos \
    2.1 \
    0.38 \
    ${bitcoind_address}

  for i in {1..30}; do
    cache_wallet_balance
    [[ $(cached_pending_outgoing) == 210000000 ]] && break
    sleep 1
  done
  [[ $(cached_pending_outgoing) == 210000000 ]] || exit 1
  [[ $(cached_effective_settled) != 0 ]] || exit 1

  bitcoin_cli -generate 2
  for i in {1..30}; do
    cache_wallet_balance
    [[ $(cached_pending_outgoing) == 0 ]] && break
    sleep 1
  done

  bitcoind_signer_balance_in_btc=$(bitcoin_signer_cli getbalance)
  bitcoind_signer_balance=$(convert_btc_to_sats "${bitcoind_signer_balance_in_btc}")
  if [[ "$(cached_effective_settled)" != "${bitcoind_signer_balance}" ]]; then
    echo "$(cached_effective_settled)" != "${bitcoind_signer_balance}"
    exit 1
  fi
}

@test "bitcoind_signer_sync: Can sweep all" {
  cache_wallet_balance
  [[ $(cached_current_settled) != 0 ]] || exit 1

  bitcoind_address=$(bitcoin_cli -regtest getnewaddress)
  bitcoin_signer_cli -named sendall recipients="[\"${bitcoind_address}\"]" fee_rate=1
  for i in {1..10}; do
    cache_wallet_balance
    [[ $(cached_current_settled) == 0 ]] \
      && [[ $(cached_pending_outgoing) != 0 ]] \
      && break
    sleep 1
  done
  [[ $(cached_current_settled) == 0 ]] \
      && [[ $(cached_pending_outgoing) != 0 ]] \
      || exit 1

  bitcoin_cli -generate 1
  for i in {1..30}; do
    cache_wallet_balance
    [[ $(cached_pending_outgoing) == 0 ]] \
      && [[ $(cached_encumbered_fees) == 0 ]] \
      && break
    sleep 1
  done
  [[ $(cached_encumbered_fees) == 0 ]] || exit 1
  [[ $(cached_effective_settled) == 0 ]] || exit 1
  [[ $(cached_pending_outgoing) == 0 ]] || exit 1
}

@test "bitcoind_signer_sync: Can spend only from unconfirmed" {
  bitcoind_signer_address=$(bitcoin_signer_cli getnewaddress)
  bitcoin_cli -regtest sendtoaddress ${bitcoind_signer_address} 1
  for i in {1..20}; do
    [[ $(bitcoin_signer_cli getunconfirmedbalance) == "1.00000000" ]] && break
    sleep 1
  done

  bitcoind_address=$(bitcoin_cli -regtest getnewaddress)
  bitcoin_signer_cli_send_all_utxos \
    0.6 \
    0.39 \
    ${bitcoind_address}

  for i in {1..30}; do
    cache_wallet_balance
    [[ $(cached_pending_outgoing) == 60000000 ]] && break
    sleep 1
  done
  [[ $(cached_pending_outgoing) == 60000000 ]] || exit 1
  [[ $(cached_effective_settled) == 0 ]] || exit 1

  bitcoin_cli -generate 2
  for i in {1..30}; do
    cache_wallet_balance
    [[ $(cached_pending_outgoing) == 0 ]] && break
    sleep 1
  done
  [[ $(cached_pending_outgoing) == 0 ]] || exit 1
  [[ $(cached_effective_settled) == $(cached_current_settled) ]] || exit 1
  bitcoind_signer_balance_in_btc=$(bitcoin_signer_cli getbalance)
  bitcoind_signer_balance=$(convert_btc_to_sats "${bitcoind_signer_balance_in_btc}")
  [[ "$(cached_effective_settled)" == "${bitcoind_signer_balance}" ]] || exit 1
}
