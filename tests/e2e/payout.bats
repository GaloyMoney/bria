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

@test "payout: Fund an address and see if the balance is reflected" {
  bria_address=$(bria_cmd new-address -w default | jq -r '.address')
  if [ -z "$bria_address" ]; then
    echo "Failed to get a new address"
    exit 1
  fi

  bitcoin_cli -regtest sendtoaddress ${bria_address} 1
  bitcoin_cli -regtest sendtoaddress ${bria_address} 1

  for i in {1..30}; do
   n_utxos=$(bria_cmd list-utxos -w default | jq '.keychains[0].utxos | length')
    [[ "${n_utxos}" == "2" ]] && break
    sleep 1
  done
  cache_default_wallet_balance
  [[ $(cached_encumbered_fees) != 0 ]] || exit 1
  [[ $(cached_pending_income) == 200000000 ]] || exit 1;
}

@test "payout: Create batch group and have two queued payouts on it" {
  bria_cmd create-batch-group --name high --interval-trigger 5
  bria_cmd queue-payout --wallet default --group-name high --destination bcrt1q208tuy5rd3kvy8xdpv6yrczg7f3mnlk3lql7ej --amount 75000000
  bria_cmd queue-payout --wallet default --group-name high --destination bcrt1q3rr02wkkvkwcj7h0nr9dqr9z3z3066pktat7kv --amount 75000000 --metadata '{"foo":{"bar":"baz"}}'

  n_payouts=$(bria_cmd list-payouts -w default | jq '.payouts | length')
  [[ "${n_payouts}" == "2" ]] || exit 1
  batch_id=$(bria_cmd list-payouts -w default | jq '.payouts[0].batchId')
  [[ "${batch_id}" == "null" ]] || exit 1
  cache_default_wallet_balance
  [[ $(cached_encumbered_outgoing) == 150000000 && $(cached_pending_outgoing) == 0 ]] || exit 1
}

@test "payout: Settling income means batch is created" {
  bitcoin_cli -generate 20

  for i in {1..30}; do
    utxo_height=$(bria_cmd list-utxos -w default | jq '.keychains[0].utxos[0].blockHeight')
    [[ "${utxo_height}" != "null" ]] && break;
    sleep 1
  done
  cache_default_wallet_balance
  [[ $(cached_pending_income) == 0 ]] || exit 1

  for i in {1..20}; do
    batch_id=$(bria_cmd list-payouts -w default | jq '.payouts[0].batchId')
    [[ "${batch_id}" != "null" ]] && break
    sleep 1
  done
  for i in {1..60}; do
    cache_default_wallet_balance
    [[ $(cached_pending_outgoing) == 150000000 ]] && break;
    sleep 1
  done

  [[ $(cached_pending_outgoing) == 150000000 ]] || exit 1
  [[ $(cached_pending_fees) != 0 ]] || exit 1
  [[ $(cached_encumbered_fees) == 0 ]] || exit 1
}

@test "payout: Add signing config to complete payout" {
    batch_id=$(bria_cmd list-payouts -w default | jq -r '.payouts[0].batchId')
    signing_failure_reason=$(bria_cmd list-signing-sessions -b "${batch_id}" | jq -r '.sessions[0].failureReason')

    [[ "${signing_failure_reason}" == "SignerConfigMissing" ]] || exit 1

    bria_cmd set-signer-config --xpub lnd_key lnd --endpoint "${LND_ENDPOINT}" --macaroon-file "./dev/lnd/regtest/lnd.admin.macaroon" --cert-file "./dev/lnd/tls.cert"
}
