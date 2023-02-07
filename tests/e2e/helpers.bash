REPO_ROOT=$(git rev-parse --show-toplevel)

bria() {
  bria_location=${REPO_ROOT}/target/debug/bria
  if [[ ! -z ${CARGO_TARGET_DIR} ]] ; then
    bria_location=${CARGO_TARGET_DIR}/debug/bria
  fi

  echo "${bria_location} $@"
  ${bria_location} $@
}

bitcoin-cli() {
  echo "docker compose exec bitcoind bitcoin-cli $@"
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
  kill -9 ${BATS_TMPDIR}/pid
}

stop_deps() {
  make clean-deps
}
