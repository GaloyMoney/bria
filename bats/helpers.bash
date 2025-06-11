REPO_ROOT=$(git rev-parse --show-toplevel)
COMPOSE_PROJECT_NAME="${COMPOSE_PROJECT_NAME:-${REPO_ROOT##*/}}"
SIGNER_ENCRYPTION_KEY="0000000000000000000000000000000000000000000000000000000000000000"
BRIA_HOME="${BRIA_HOME:-.bria}"
export PG_CON="${PG_CON:-${DATABASE_URL}}"
if [[ "${BRIA_CONFIG}" == "docker" ]]; then
  COMPOSE_FILE_ARG="-f docker-compose.yml"
fi
BITCOIND_SIGNER_ENDPOINT="${BITCOIND_SIGNER_ENDPOINT:-https://localhost:18543}"
SATS_IN_ONE_BTC=100000000

bria_cmd() {
  bria_location=${REPO_ROOT}/target/debug/bria
  if [[ ! -z ${CARGO_TARGET_DIR} ]] ; then
    bria_location=${CARGO_TARGET_DIR}/debug/bria
  fi

  ${bria_location} $@
}

cache_wallet_balance() {
  local wallet_name="${1:-default}"
  balance=$(bria_cmd wallet-balance -w "${wallet_name}")
}

cached_pending_income() {
  echo ${balance} | jq -r '.utxoPendingIncoming'
}

cached_encumbered_fees() {
  echo ${balance} | jq -r '.feesEncumbered'
}

cached_current_settled() {
  echo ${balance} | jq -r '.utxoSettled'
}

cached_effective_settled() {
  echo ${balance} | jq -r '.effectiveSettled'
}

cached_pending_outgoing() {
  echo ${balance} | jq -r '.effectivePendingOutgoing'
}

cached_pending_fees() {
  echo ${balance} | jq -r '.feesPending'
}

cached_encumbered_outgoing() {
  echo ${balance} | jq -r '.effectiveEncumberedOutgoing'
}

bitcoin_cli() {
  docker exec "${COMPOSE_PROJECT_NAME}-bitcoind-1" bitcoin-cli $@
}

bitcoin_signer_cli() {
  docker exec "${COMPOSE_PROJECT_NAME}-bitcoind-signer-1" bitcoin-cli $@
}

convert_btc_to_sats() {
  echo "$1 * $SATS_IN_ONE_BTC / 1" | bc
}

bitcoin_signer_cli_send_all_utxos () {
  amount=$1
  change=$2
  send_address=$3

  rawtx_utxos=$(bitcoin_signer_cli listunspent 0 | jq -c '[.[] | {txid: .txid, vout: .vout}]')

  change_address=$(bitcoin_signer_cli getrawchangeaddress "bech32")
  rawtx_addresses="[{\"${send_address}\":$amount},{\"${change_address}\":$change}]"

  unsigned_tx=$(bitcoin_signer_cli createrawtransaction $rawtx_utxos $rawtx_addresses)
  signed_tx=$(bitcoin_signer_cli signrawtransactionwithwallet $unsigned_tx | jq -r '.hex')
  bitcoin_signer_cli sendrawtransaction $signed_tx
}


lnd_cli() {
  docker exec "${COMPOSE_PROJECT_NAME}-lnd-1" lncli -n regtest $@
}

reset_pg() {
  docker exec "${COMPOSE_PROJECT_NAME}-postgres-1" psql $PG_CON -c "DROP SCHEMA public CASCADE"
  docker exec "${COMPOSE_PROJECT_NAME}-postgres-1" psql $PG_CON -c "CREATE SCHEMA public"
}

restart_bitcoin_stack() {
  docker compose ${COMPOSE_FILE_ARG} rm -sfv bitcoind bitcoind-signer lnd fulcrum mempool || true
  # Running this twice has sometimes bitcoind is dangling in CI
  docker compose ${COMPOSE_FILE_ARG} rm -sfv bitcoind bitcoind-signer lnd fulcrum mempool || true
  docker compose ${COMPOSE_FILE_ARG} up -d bitcoind bitcoind-signer lnd fulcrum mempool
  retry 10 1 lnd_cli getinfo
}

bitcoind_init() {
  local wallet="${1:-default}"

  bitcoin_cli createwallet "default" || true
  bitcoin_cli generatetoaddress 200 "$(bitcoin_cli getnewaddress)"

  if [[ "${wallet}" == "default" ]]; then 
    bitcoin_signer_cli createwallet "default" || true
    bitcoin_signer_cli -rpcwallet=default importdescriptors "$(cat ${REPO_ROOT}/tests/e2e/bitcoind_signer_descriptors.json)"
  elif [[ "${wallet}" == "multisig" ]]; then
    bitcoin_signer_cli createwallet "multisig" || true 
    bitcoin_signer_cli -rpcwallet=multisig importdescriptors "$(cat ${REPO_ROOT}/tests/e2e/bitcoind_multisig_signer_descriptors.json)"
    bitcoin_signer_cli createwallet "multisig2" || true 
    bitcoin_signer_cli -rpcwallet=multisig2 importdescriptors "$(cat ${REPO_ROOT}/tests/e2e/bitcoind_multisig2_signer_descriptors.json)"
  fi
}

start_daemon() {
  SIGNER_ENCRYPTION_KEY="${SIGNER_ENCRYPTION_KEY}" background bria_cmd daemon --config ./tests/e2e/bria.${BRIA_CONFIG:-local}.yml run > .e2e-logs
  for i in {1..20}
  do
    if head .e2e-logs | grep -q 'Starting main server on port'; then
      break
    else
      sleep 1
    fi
  done
}

stop_daemon() {
  if [[ -f ${BRIA_HOME}/daemon-pid ]]; then
    kill -9 $(cat ${BRIA_HOME}/daemon-pid) || true
  fi
}

bria_init() {
  local wallet_type="${1:-default}"
  
  if [[ "${BRIA_CONFIG}" == "docker" ]]; then
    retry_cmd="retry 10 1"
  else
    retry_cmd=""
  fi

  $retry_cmd bria_cmd admin bootstrap

  bria_cmd admin create-account -n default
  
  if [[ "${wallet_type}" == "default" ]]; then
    $retry_cmd bria_cmd create-wallet -n default descriptors -d "wpkh([6f2fa1b2/84'/0'/0']tpubDDDDGYiFda8HfJRc2AHFJDxVzzEtBPrKsbh35EaW2UGd5qfzrF2G87ewAgeeRyHEz4iB3kvhAYW1sH6dpLepTkFUzAktumBN8AXeXWE9nd1/0/*)#l6n08zmr" \
      -c "wpkh([6f2fa1b2/84'/0'/0']tpubDDDDGYiFda8HfJRc2AHFJDxVzzEtBPrKsbh35EaW2UGd5qfzrF2G87ewAgeeRyHEz4iB3kvhAYW1sH6dpLepTkFUzAktumBN8AXeXWE9nd1/1/*)#wwkw6htm"
  elif [[ "${wallet_type}" == "multisig" ]]; then
    local key1="tpubDEaDfeS1EXpqLVASNCW7qAHW1TFPBpk2Z39gUXjFnsfctomZ7N8iDpy6RuGwqdXAAZ5sr5kQZrxyuEn15tqPJjM4mcPSuXzV27AWRD3p9Q4"
    local key2="tpubDEPCxBfMFRNdfJaUeoTmepLJ6ZQmeTiU1Sko2sdx1R3tmPpZemRUjdAHqtmLfaVrBg1NBx2Yx3cVrsZ2FTyBuhiH9mPSL5ozkaTh1iZUTZx"
    
    $retry_cmd bria_cmd import-xpub -x "${key1}" -n key1 -d m/48h/1h/0h/2h
    bria_cmd import-xpub -x "${key2}" -n key2 -d m/48h/1h/0h/2h
    bria_cmd create-wallet -n multisig sorted-multisig -x key1 key2 -t 2
  fi

  echo "Bria Initialization Complete"
}

bria_lnd_init() {
  retry 10 1 bria_cmd admin bootstrap
  bria_cmd admin create-account -n default
  retry 10 1 bria_cmd import-xpub -x tpubDD4vFnWuTMEcZiaaZPgvzeGyMzWe6qHW8gALk5Md9kutDvtdDjYFwzauEFFRHgov8pAwup5jX88j5YFyiACsPf3pqn5hBjvuTLRAseaJ6b4 -n lnd_key -d m/84h/0h/0h
  bria_cmd create-wallet -n default wpkh -x lnd_key

  echo "Bria Initialization Complete"
}


# Run the given command in the background. Useful for starting a
# node and then moving on with commands that exercise it for the
# test.
#
# Ensures that BATS' handling of file handles is taken into account;
# see
# https://github.com/bats-core/bats-core#printing-to-the-terminal
# https://github.com/sstephenson/bats/issues/80#issuecomment-174101686
# for details.
background() {
  "$@" 3>- &
  echo $!
}

# Taken from https://github.com/docker/swarm/blob/master/test/integration/helpers.bash
# Retry a command $1 times until it succeeds. Wait $2 seconds between retries.
retry() {
  local attempts=$1
  shift
  local delay=$1
  shift
  local i

  for ((i=0; i < attempts; i++)); do
    run "$@"
    if [[ "$status" -eq 0 ]] ; then
      return 0
    fi
    sleep "$delay"
  done

  echo "Command \"$*\" failed $attempts times. Output: $output"
  false
}
