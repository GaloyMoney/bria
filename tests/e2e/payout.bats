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

@test "payout: Batch inclusion and payout cancellation" {
  bria_cmd create-payout-queue --name high --interval-trigger 5
  payout_id=$(bria_cmd submit-payout -w default --queue-name high --destination bcrt1q208tuy5rd3kvy8xdpv6yrczg7f3mnlk3lql7ej --amount 75000000 | jq -r '.id')
  for i in {1..20}; do
    cache_wallet_balance
    [[ $(cached_encumbered_outgoing) == 75000000 ]] && break;
    sleep 1
  done
  [[ $(cached_encumbered_outgoing) == 75000000 ]] || exit 1

  estimated_at=$(bria_cmd get-payout --id ${payout_id} | jq -r '.payout.batchInclusionEstimatedAt')
  [[ "${estimated_at}" != "null" ]] || exit 1

  bria_cmd cancel-payout --id ${payout_id}

  estimated_at=$(bria_cmd get-payout --id ${payout_id} | jq -r '.payout.batchInclusionEstimatedAt')
  [[ "${estimated_at}" = "null" ]] || exit 1

  for i in {1..20}; do
    cache_wallet_balance
    [[ $(cached_encumbered_outgoing) == 0 ]] && break;
    sleep 1
  done
  [[ $(cached_encumbered_outgoing) == 0 ]] || exit 1;
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
    [[ "${n_utxos}" == "3" ]] && break
    sleep 1
  done
  cache_wallet_balance
  [[ $(cached_encumbered_fees) != 0 ]] || exit 1
  [[ $(cached_pending_income) == 200000000 ]] || exit 1;
}

@test "payout: Create payout queue and have two queued payouts on it" {
  bria_cmd submit-payout --wallet default --queue-name high --destination bcrt1q208tuy5rd3kvy8xdpv6yrczg7f3mnlk3lql7ej --amount 75000000
  bria_cmd submit-payout --wallet default --queue-name high --destination bcrt1q3rr02wkkvkwcj7h0nr9dqr9z3z3066pktat7kv --amount 75000000 --metadata '{"foo":{"bar":"baz"}}'

  n_payouts=$(bria_cmd list-payouts -w default | jq '.payouts | length')
  [[ "${n_payouts}" == "3" ]] || exit 1
  batch_id=$(bria_cmd list-payouts -w default | jq '.payouts[0].batchId')
  [[ "${batch_id}" == "null" ]] || exit 1
  cache_wallet_balance
  [[ $(cached_encumbered_outgoing) == 150000000 && $(cached_pending_outgoing) == 0 ]] || exit 1
}

@test "payout: Settling income means batch is created" {
  bitcoin_cli -generate 20

  for i in {1..30}; do
    utxo_height=$(bria_cmd list-utxos -w default | jq '.keychains[0].utxos[0].blockHeight')
    [[ "${utxo_height}" != "null" ]] && break;
    sleep 1
  done
  cache_wallet_balance
  [[ $(cached_pending_income) == 0 ]] || exit 1

  for i in {1..20}; do
    batch_id=$(bria_cmd list-payouts -w default | jq -r '.payouts[1].batchId')
    [[ "${batch_id}" != "null" ]] && break
    sleep 1
  done
  [[ "${batch_id}" != "null" ]] || exit 1
  for i in {1..20}; do
    cache_wallet_balance
    [[ $(cached_pending_outgoing) == 150000000 ]] && break;
    sleep 1
  done

  [[ $(cached_pending_outgoing) == 150000000 ]] || exit 1
  [[ $(cached_pending_fees) != 0 ]] || exit 1
  [[ $(cached_encumbered_fees) == 0 ]] || exit 1
}

@test "payout: Add signing config to complete payout" {
  batch_id=$(bria_cmd list-payouts -w default | jq -r '.payouts[1].batchId')
  for i in {1..20}; do
    signing_failure_reason=$(bria_cmd get-batch -b "${batch_id}" | jq -r '.signingSessions[0].failureReason')
    [[ "${signing_failure_reason}" == "SignerConfigMissing" ]] && break
    sleep 1
  done

  [[ "${signing_failure_reason}" == "SignerConfigMissing" ]] || exit 1

  cache_wallet_balance
  [[ $(cached_pending_income) == 0 ]] || exit 1

  bria_cmd set-signer-config \
    --xpub "68bfb290" bitcoind \
    --endpoint "${BITCOIND_SIGNER_ENDPOINT}" \
    --rpc-user "rpcuser" \
    --rpc-password "rpcpassword"

  for i in {1..20}; do
    signing_status=$(bria_cmd get-batch -b "${batch_id}" | jq -r '.signingSessions[0].state')
    [[ "${signing_status}" == "Complete" ]] && break
    sleep 1
  done
  if [[ "${signing_status}" != "Complete" ]]; then
    signing_failure_reason=$(bria_cmd get-batch -b "${batch_id}" | jq -r '.signingSessions[0].failureReason')
    echo "signing_status: ${signing_status}"
    echo "signing_failure_reason: ${signing_failure_reason}"
  fi

  for i in {1..20}; do
    cache_wallet_balance
    [[ $(cached_pending_income) != 0 ]] && break;
    sleep 1
  done

  [[ $(cached_pending_income) != 0 ]] || exit 1
  [[ $(cached_current_settled) == 0 ]] || exit 1
  bitcoin_cli -generate 2

  for i in {1..20}; do
    cache_wallet_balance
    [[ $(cached_current_settled) != 0 ]] && break;
    sleep 1
  done

  [[ $(cached_current_settled) != 0 ]] || exit 1;
}

@test "payout: Creates a manually triggered payout-queue and triggers it" {
  bria_address=$(bria_cmd new-address -w default | jq -r '.address')
  bitcoin_cli -regtest sendtoaddress ${bria_address} 1
  bitcoin_cli -generate 10
  bria_cmd create-payout-queue -n manual -m true
  bria_cmd submit-payout --wallet default --queue-name manual --destination bcrt1q208tuy5rd3kvy8xdpv6yrczg7f3mnlk3lql7ej --amount 75000000

  for i in {1..20}; do
    batch_id=$(bria_cmd list-payouts -w default | jq -r '.payouts[0].batchId')
     [[ "${batch_id}" != "null" ]] && break;
    sleep 1
  done
  [[ "${batch_id}" == "null" ]] || exit 1

  bria_cmd trigger-payout-queue --name manual;

  for i in {1..20}; do
    payout=$(bria_cmd list-payouts -w default | jq -r '.payouts[0]')
    payout_id=$(echo ${payout} | jq -r '.id')
    batch_id=$(echo ${payout} | jq -r '.batchId')
    tx_id=$(echo ${payout} | jq -r '.txId')
    vout=$(echo ${payout} | jq -r '.vout')
    [[ "${batch_id}" != "null" && "${tx_id}" != "null" && "${vout}" != "null" ]] && break
    sleep 1
  done
  [[ "${batch_id}" != "null" && "${tx_id}" != "null" && "${vout}" != "null" ]] || exit 1

  payout=$(bria_cmd get-payout --id ${payout_id} | jq -r '.payout')
  batch_id=$(echo ${payout} | jq -r '.batchId')
  tx_id=$(echo ${payout} | jq -r '.txId')
  vout=$(echo ${payout} | jq -r '.vout')
  [[ "${batch_id}" != "null" && "${tx_id}" != "null" && "${vout}" != "null" ]] || exit 1

  for i in {1..20}; do
    cache_wallet_balance
    [[ $(cached_pending_income) != 0 ]] && break;
    echo $(bria_cmd wallet-balance -w default)
    sleep 1
  done
  [[ $(cached_pending_income) != 0 ]] || exit 1

  bitcoin_cli -generate 2

  for i in {1..20}; do
    cache_wallet_balance
    [[ $(cached_pending_income) == 0 ]] && break;
    sleep 1
  done
  [[ $(cached_pending_income) == 0 ]] || exit 1;
}

@test "payout: Can send to another wallet" {
  local key="tpubDEPCxBfMFRNdfJaUeoTmepLJ6ZQmeTiU1Sko2sdx1R3tmPpZemRUjdAHqtmLfaVrBg1NBx2Yx3cVrsZ2FTyBuhiH9mPSL5ozkaTh1iZUTZx"

  bria_cmd import-xpub -x "${key}" -n other -d m/48h/1h/0h/2h
  bria_cmd create-wallet -n other wpkh -x other

  bria_cmd submit-payout -w default \
    --queue-name high \
    --destination other \
    --amount 70000000 \
    --metadata '{"transfer":true}'

  transfer_metadata=$(bria_cmd list-addresses -w other | jq -r '.addresses[0].metadata.transfer')

  [[ "${transfer_metadata}" == "true" ]] || exit 1

  for i in {1..20}; do
    cache_wallet_balance
    [[ $(cached_pending_outgoing) == 70000000 ]] && break;
    sleep 1
  done
  [[ $(cached_pending_outgoing) == 70000000 ]] || exit 1;
}

@test "payout: Can CPFP when enabled in payout queue" {
  for i in {1..20}; do
    available_utxos=$(bria_cmd list-utxos -w default | jq -r '.keychains[0].utxos | length')
    [[ "${available_utxos}" == "1" ]] && break
    sleep 1
  done
  [[ "${available_utxos}" == "1" ]] || exit 1

  for i in {1..20}; do
    block_height=$(bria_cmd list-utxos -w default | jq -r '.keychains[0].utxos[0].blockHeight')
    [[ "${block_height}" == "null" ]] && break
    sleep 1
  done
  [[ "${block_height}" == "null" ]] || exit 1

  bria_cmd submit-payout -w default \
    --queue-name high \
    --destination bcrt1q208tuy5rd3kvy8xdpv6yrczg7f3mnlk3lql7ej \
    --amount 100000

  for i in {1..20}; do
    cache_wallet_balance
    [[ $(cached_encumbered_outgoing) == 100000 ]] && break;
    sleep 1
  done
  [[ $(cached_encumbered_outgoing) == 100000 ]] || exit 1;

  batch_id=$(bria_cmd list-payouts -w default | jq -r '.payouts[0].batchId')
  [[ "${batch_id}" == "null" ]] || exit 1

  queue_id=$(bria_cmd list-payout-queues | jq -r '.PayoutQueues[] | select(.name == "high").id')
  bria_cmd update-payout-queue -i "${queue_id}" --interval-trigger 5 --cpfp-after-mins 0

  for i in {1..90}; do
    batch_id=$(bria_cmd list-payouts -w default | jq -r '.payouts[0].batchId')
    [[ "${batch_id}" != "null" ]] && break
    sleep 1
  done
  [[ "${batch_id}" != "null" ]] || exit 1;

  cache_wallet_balance
  [[ $(cached_encumbered_outgoing) == 0 ]] && break;
}

@test "payout: Create and cancel an unsigned batch" {
  # invalidates signer to allow cancel the batch
  bria_cmd set-signer-config \
    --xpub "68bfb290" bitcoind \
    --endpoint "${BITCOIND_SIGNER_ENDPOINT}" \
    --rpc-user "rpcuser" \
    --rpc-password "invalidpassword"

  bria_address=$(bria_cmd new-address -w default | jq -r '.address')
  bitcoin_cli -regtest sendtoaddress ${bria_address} 1
  bitcoin_cli -generate 10

  bria_cmd create-payout-queue -n cancel_queue -m true
  payout_id=$(bria_cmd submit-payout -w default --queue-name cancel_queue --destination bcrt1q208tuy5rd3kvy8xdpv6yrczg7f3mnlk3lql7ej --amount 1300000 | jq -r '.id')

  # Wait for payout to be encumbered
  for i in {1..20}; do
    cache_wallet_balance
    [[ $(cached_encumbered_outgoing) == 1300000 && $(cached_effective_settled) -ge 100000000 ]] && break
    sleep 2
  done
  [[ $(cached_encumbered_outgoing) == 1300000 && $(cached_effective_settled) -ge 100000000 ]] || exit 1
  effective_settled=$(cached_effective_settled)

  # Wait for the batch to be created
  for i in {1..20}; do
    bria_cmd trigger-payout-queue --name cancel_queue
    batch_id=$(bria_cmd get-payout -i "${payout_id}" | jq -r '.payout.batchId')
    [[ "${batch_id}" != "null" ]] && break
    sleep 2
  done
  [[ "${batch_id}" != "null" ]] || exit 1

  # Verify the batch exists
  batch=$(bria_cmd get-batch -b "${batch_id}")
  [[ $(echo ${batch} | jq -r '.id') == "${batch_id}" && $(echo ${batch} | jq -r '.cancelled') == "false" ]] || exit 1

  # Cancel the batch
  bria_cmd cancel-batch --batch-id "${batch_id}"

  # Verify the payout is marked as cancelled
  for i in {1..20}; do
    payout=$(bria_cmd get-payout -i ${payout_id} | jq -r '.payout')
    batch_id_after=$(echo ${payout} | jq -r '.batchId')
    cancelled=$(echo ${payout} | jq -r '.cancelled')
    [[ "${batch_id_after}" == "${batch_id}" && "${cancelled}" == "true" ]] && break
    sleep 1
  done
  [[ "${batch_id_after}" == "${batch_id}" && "${cancelled}" == "true" ]] || exit 1

  # Verify the batch is marked as cancelled
  batch=$(bria_cmd get-batch -b "${batch_id}")
  [[ $(echo ${batch} | jq -r '.id') == "${batch_id}" && $(echo ${batch} | jq -r '.cancelled') == "true" ]] || exit 1

  # Check that the funds are no longer encumbered
  for i in {1..20}; do
    cache_wallet_balance
    [[ $(cached_encumbered_outgoing) == 0 && $(cached_effective_settled) == ${effective_settled} ]] && break
    sleep 1
  done
  [[ $(cached_encumbered_outgoing) == 0 ]] || exit 1
  [[ $(cached_effective_settled) == ${effective_settled} ]] || exit 1
}

@test "payout: Error when try to create and cancel a signed batch" {
  bria_cmd set-signer-config \
    --xpub "68bfb290" bitcoind \
    --endpoint "${BITCOIND_SIGNER_ENDPOINT}" \
    --rpc-user "rpcuser" \
    --rpc-password "rpcpassword"

  bria_address=$(bria_cmd new-address -w default | jq -r '.address')
  bitcoin_cli -regtest sendtoaddress ${bria_address} 1
  bitcoin_cli -generate 10

  bria_cmd create-payout-queue -n cancel_queue -m true || true
  payout_id=$(bria_cmd submit-payout -w default --queue-name cancel_queue --destination bcrt1q208tuy5rd3kvy8xdpv6yrczg7f3mnlk3lql7ej --amount 1300000 | jq -r '.id')

  # Wait for payout to be encumbered
  for i in {1..20}; do
    cache_wallet_balance
    [[ $(cached_encumbered_outgoing) == 1300000 && $(cached_effective_settled) -ge 100000000 ]] && break
    sleep 2
  done
  [[ $(cached_encumbered_outgoing) == 1300000 && $(cached_effective_settled) -ge 100000000 ]] || exit 1

  # Wait for the batch to be created
  for i in {1..20}; do
    bria_cmd trigger-payout-queue --name cancel_queue
    batch_id=$(bria_cmd get-payout -i "${payout_id}" | jq -r '.payout.batchId')
    [[ "${batch_id}" != "null" ]] && break
    sleep 2
  done
  [[ "${batch_id}" != "null" ]] || exit 1

  # Verify the batch exists
  batch=$(bria_cmd get-batch -b "${batch_id}")
  [[ $(echo ${batch} | jq -r '.id') == "${batch_id}" ]] || exit 1

  # Try to cancel the batch
  run bria_cmd cancel-batch --batch-id "${batch_id}"
  [[ "$status" -ne 0 ]]
  [[ "$output" == *"BatchError - Batch is already signed and can't be cancelled"* ]]

  # Check that the funds are no longer encumbered
  for i in {1..20}; do
    cache_wallet_balance
    [[ $(cached_encumbered_outgoing) == 0 ]] && break
    sleep 1
  done
  [[ $(cached_encumbered_outgoing) == 0 ]] || exit 1

  # Verify the batch is not marked as cancelled
  batch=$(bria_cmd get-batch -b "${batch_id}")
  [[ $(echo ${batch} | jq -r '.id') == "${batch_id}" && $(echo ${batch} | jq -r '.cancelled') == "false" ]] || exit 1
}
