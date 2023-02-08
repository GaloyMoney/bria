REPO_ROOT=$(git rev-parse --show-toplevel)

bria() {
  bria_location=${REPO_ROOT}/target/debug/bria
  if [[ ! -z ${CARGO_TARGET_DIR} ]] ; then
    bria_location=${CARGO_TARGET_DIR}/debug/bria
  fi

  ${bria_location} $@
}

bitcoin-cli() {
  docker compose exec bitcoind bitcoin-cli $@
}

background() {
  "$@" 3>- &
  echo $!
}

start_deps() {
  make clean-deps
  make start-deps
  sleep 3
  make setup-db
}

start_daemon() {
  background bria daemon
  echo $! > ${BATS_TMPDIR}/pid
}

stop_daemon() {
  kill -9 $(cat ${BATS_TMPDIR}/pid)
}

stop_deps() {
  make clean-deps
}

bria_init() {
  bria admin bootstrap
  bria admin create-account -n default

	bitcoin-cli createwallet "default"
	bitcoin-cli -generate 200
  bria import-xpub -x tpubDDEGUyCLufbxAfQruPHkhUcu55UdhXy7otfcEQG4wqYNnMfq9DbHPxWCqpEQQAJUDi8Bq45DjcukdDAXasKJ2G27iLsvpdoEL5nTRy5TJ2B -n key1 -d m/64h/1h/0
	bria create-wallet -n default -x key1
  echo "Bria Initialization Complete"
}
