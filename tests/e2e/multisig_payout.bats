#!/usr/bin/env bats

load "helpers"

setup_file() {
  restart_bitcoin_stack
  reset_pg
  bitcoind_init multisig
  start_daemon
  bria_init multisig
}

teardown_file() {
  stop_daemon
}

@test "multisig_payout: Fund an address and see if the balance is reflected" {
  bria_address=$(bria_cmd new-address -w multisig | jq -r '.address')
  
  if [ -z "$bria_address" ]; then
    echo "Failed to get a new address"
    exit 1
  fi

  bitcoin_cli -regtest sendtoaddress ${bria_address} 1

  for i in {1..30}; do
    n_utxos=$(bria_cmd list-utxos -w multisig | jq '.keychains[0].utxos | length')
    [[ "${n_utxos}" == "1" ]] && break
    sleep 1
  done
  
  cache_wallet_balance multisig
  [[ $(cached_encumbered_fees) != 0 ]] || exit 1
  [[ $(cached_pending_income) == 100000000 ]] || exit 1;
}

@test "mutlisig_payout: Create payout queue and have a queued payout on it" {
  bria_cmd create-payout-queue --name high --interval-trigger 5
  bria_cmd submit-payout --wallet multisig --queue-name high --destination bcrt1q208tuy5rd3kvy8xdpv6yrczg7f3mnlk3lql7ej --amount 75000000

  n_payouts=$(bria_cmd list-payouts -w multisig | jq '.payouts | length')
  [[ "${n_payouts}" == "1" ]] || exit 1
  batch_id=$(bria_cmd list-payouts -w multisig | jq '.payouts[0].batchId')
  [[ "${batch_id}" == "null" ]] || exit 1
  
  cache_wallet_balance multisig
  [[ $(cached_encumbered_outgoing) == 75000000 && $(cached_pending_outgoing) == 0 ]] || exit 1
}

@test "multisig_payout: Signing unsigned psbt and submitting signed psbt" {
  bitcoin_cli -generate 5

  for i in {1..20}; do
    batch_id=$(bria_cmd list-payouts -w multisig | jq -r '.payouts[0].batchId')
    [[ "${batch_id}" != "null" ]] && break
    sleep 1
  done
  
  [[ "${batch_id}" != "null" ]] || exit 1

  unsigned_psbt=$(bria_cmd get-batch -b "${batch_id}" | jq -r '.unsignedPsbt')
  signed_psbt=$(bitcoin_signer_cli -rpcwallet=multisig walletprocesspsbt "${unsigned_psbt}" true ALL true | jq -r '.psbt')
  bria_cmd submit-signed-psbt -b "${batch_id}" -x key1 -s "${signed_psbt}"
  bria_cmd set-signer-config \
    --xpub key2 bitcoind \
    --endpoint "${BITCOIND_SIGNER_ENDPOINT}"/wallet/multisig2 \
    --rpc-user "rpcuser" \
    --rpc-password "rpcpassword"
  
  for i in {1..20}; do
    signing_status=$(bria_cmd get-batch -b "${batch_id}" | jq -r '.sessions[0].state')
    [[ "${signing_status}" == "Complete" ]] && break
    sleep 1
  done
  
  if [[ "${signing_status}" != "Complete" ]]; then
    signing_failure_reason=$(bria_cmd get-batch -b "${batch_id}" | jq -r '.signingSessions[0].failureReason')
    echo "signing_status: ${signing_status}"
    echo "signing_failure_reason: ${signing_failure_reason}"
  fi

  for i in {1..20}; do
    cache_wallet_balance multisig
    [[ $(cached_pending_income) != 0 ]] && break;
    sleep 1
  done

  [[ $(cached_pending_income) != 0 ]] || exit 1
  [[ $(cached_current_settled) == 0 ]] || exit 1
  bitcoin_cli -generate 2

  for i in {1..20}; do
    cache_wallet_balance multisig
    [[ $(cached_current_settled) != 0 ]] && break;
    sleep 1
  done

  [[ $(cached_current_settled) != 0 ]] || exit 1;
}
