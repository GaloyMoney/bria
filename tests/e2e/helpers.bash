REPO_ROOT=$(git rev-parse --show-toplevel)

bria_cmd() {
  bria_location=${REPO_ROOT}/target/debug/bria
  if [[ ! -z ${CARGO_TARGET_DIR} ]] ; then
    bria_location=${CARGO_TARGET_DIR}/debug/bria
  fi

  ${bria_location} $@
}

cache_default_wallet_balance() {
  balance=$(bria_cmd wallet-balance -w default --json)
}

cached_pending_income() {
  echo ${balance} | jq -r '.pending_incoming'
}

cached_encumbered_fees() {
  echo ${balance} | jq -r '.encumbered_fees'
}

cached_current_settled() {
  echo ${balance} | jq -r '.current_settled'
}

cached_pending_outgoing() {
  echo ${balance} | jq -r '.pending_outgoing'
}

cached_encumbered_outgoing() {
  echo ${balance} | jq -r '.encumbered_outgoing'
}

bitcoin_cli() {
  docker exec bria-bitcoind-1 bitcoin-cli $@
}

reset_pg() {
  docker exec bria-postgres-1 psql $PG_CON -c "DROP SCHEMA public CASCADE"
  docker exec bria-postgres-1 psql $PG_CON -c "CREATE SCHEMA public"
}

bitcoind_init() {
  bitcoin_cli createwallet "default" || true
	bitcoin_cli -generate 200
}

start_daemon() {
  background bria_cmd daemon --config ./tests/e2e/bria.${BRIA_CONFIG:-local}.yml > logs
  sleep 5 # wait for daemon to be up and running
}

stop_daemon() {
  if [[ -f .bria/daemon_pid ]]; then
    kill -9 $(cat .bria/daemon_pid) || true
  fi
}

bria_init() {
  bria_cmd admin bootstrap
  bria_cmd admin create-account -n default
  bria_cmd import-xpub -x tpubDDEGUyCLufbxAfQruPHkhUcu55UdhXy7otfcEQG4wqYNnMfq9DbHPxWCqpEQQAJUDi8Bq45DjcukdDAXasKJ2G27iLsvpdoEL5nTRy5TJ2B -n key1 -d m/64h/1h/0
	bria_cmd create-wallet -n default -x key1

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
