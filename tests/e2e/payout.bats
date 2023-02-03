#!/usr/bin/env bats

load "helpers"

setup_file() {
  echo 'Setup File'
  start_deps
  start_daemon
  sleep 2
}

teardown_file() {
  stop_daemon
  stop_deps
}

@test "Fund an Address" {
  bria admin bootstrap
  bria admin create-account -n default
	docker compose exec bitcoind bitcoin-cli createwallet "default"
	docker compose exec bitcoind bitcoin-cli -generate 200
  bria import-xpub -x tpubDDEGUyCLufbxAfQruPHkhUcu55UdhXy7otfcEQG4wqYNnMfq9DbHPxWCqpEQQAJUDi8Bq45DjcukdDAXasKJ2G27iLsvpdoEL5nTRy5TJ2B -n key1 -d m/64h/1h/0
	bria create-wallet -n default -x key1
  docker compose exec bitcoind bitcoin-cli -regtest sendtoaddress bcrt1q0k9yhm4jpqz9srfggvjsqt8f2gjcqu794h0sww 50
	docker compose exec bitcoind bitcoin-cli -generate 1
  for i in {1..60}; do
    bria wallet-balance -w default
    sleep 1
  done
  exit 1
}
