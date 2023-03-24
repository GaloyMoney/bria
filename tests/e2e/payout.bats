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

@test "Fund an address and see if the balance is reflected" {
  bria_address=$(bria_cmd new-address -w default --raw)
  if [ -z "$bria_address" ]; then
    echo "Failed to get a new address"
    exit 1
  fi

  bitcoin_cli -regtest sendtoaddress ${bria_address} 1
  bitcoin_cli -regtest sendtoaddress ${bria_address} 1

  for i in {1..30}; do
    cache_default_wallet_balance
    [[ $(cached_pending_income) == 200000000 ]] && break
    sleep 1
  done
  [[ $(cached_encumbered_fees) == 0 ]] || exit 1
  [[ $(cached_pending_income) == 200000000 ]] || exit 1;
}

@test "Create batch group and have two queued payouts on it" {
  bria_cmd create-batch-group --name high
  bria_cmd queue-payout --wallet default --group-name high --destination bcrt1q208tuy5rd3kvy8xdpv6yrczg7f3mnlk3lql7ej --amount 75000000
  bria_cmd queue-payout --wallet default --group-name high --destination bcrt1q3rr02wkkvkwcj7h0nr9dqr9z3z3066pktat7kv --amount 75000000

  for i in {1..30}; do
    cache_default_wallet_balance
    [[ $(cached_encumbered_outgoing) == 150000000 && $(cached_pending_outgoing) == 0 ]] && break
    sleep 1
  done
  [[ $(cached_encumbered_outgoing) == 150000000 && $(cached_pending_outgoing) == 0 ]] || exit 1
}

@test "Blocks settle income and makes outgoing pending" {
  bitcoin_cli -generate 20

  for i in {1..30}; do
    cache_default_wallet_balance
    [[ $(cached_pending_income) == 0 ]] && break
    sleep 1
  done
  [[ $(cached_current_settled) == 200000000 ]] || exit 1
  [[ $(cached_encumbered_fees) != 0 ]] || exit 1

  for i in {1..120}; do
    cache_default_wallet_balance
    [[ $(cached_pending_outgoing) == 150000000 ]] && break
    sleep 1
  done
  [[ $(cached_pending_outgoing) == 150000000 ]] || exit 1
  [[ $(cached_pending_fees) != 0 ]] || exit 1
  [[ $(cached_encumbered_fees) == 0 ]] || exit 1
}

@test "Outgoing unconfirmed utxo" {
  bria_cmd queue-payout --wallet default --group-name high --destination bcrt1q208tuy5rd3kvy8xdpv6yrczg7f3mnlk3lql7ej --amount 75000000
  bria_address=$(bria_cmd new-address -w default --raw)
  bitcoin_cli -regtest sendtoaddress ${bria_address} 1

  for i in {1..30}; do
    cache_default_wallet_balance
    [[ $(cached_pending_income) == 100000000 ]] && break
    sleep 1
  done
  [[ $(cached_pending_outgoing) == 75000000 ]] && break
  [[ $(cached_current_settled) == 0 ]] && break

  bitcoin_cli -generate 3

  for i in {1..30}; do
    cache_default_wallet_balance
    [[ $(cached_pending_income) == 0 ]] && break
    sleep 1
  done
  [[ $(cached_encumbered_fees) == 0 ]] || exit 1
}
