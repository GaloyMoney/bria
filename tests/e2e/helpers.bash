REPO_ROOT=$(git rev-parse --show-toplevel)

bria() {
  bria_location=${REPO_ROOT}/target/debug/bria
  if [[ ! -z ${CARGO_TARGET_DIR} ]] ; then
    bria_location=${CARGO_TARGET_DIR}/debug/bria
  fi

  ${bria_location} $@
}

bitcoin-cli() {
  docker exec bria-bitcoind-1 bitcoin-cli $@
}

background() {
  "$@" &
}

reset_pg() {
  docker exec bria-postgres-1 psql $PG_CON -c "DROP SCHEMA public CASCADE"
  docker exec bria-postgres-1 psql $PG_CON -c "CREATE SCHEMA public"
  cargo sqlx migrate run
}

bitcoind_init() {
  bitcoin-cli createwallet "default" || true
	bitcoin-cli -generate 200
}

start_daemon() {
  background bria daemon
  sleep 5 # wait for daemon to be up and running
}

stop_daemon() {
  if [[ -f .bria/daemon_pid ]]; then
    kill -9 $(cat .bria/daemon_pid)
  fi
}

bria_init() {
  bria admin bootstrap
  bria admin create-account -n default
  bria import-xpub -x tpubDDEGUyCLufbxAfQruPHkhUcu55UdhXy7otfcEQG4wqYNnMfq9DbHPxWCqpEQQAJUDi8Bq45DjcukdDAXasKJ2G27iLsvpdoEL5nTRy5TJ2B -n key1 -d m/64h/1h/0
	bria create-wallet -n default -x key1

  echo "Bria Initialization Complete"
}
