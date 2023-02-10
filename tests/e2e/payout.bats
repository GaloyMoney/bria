#!/usr/bin/env bats

load "helpers"

setup_file() {
  reset_pg
  bitcoind_init
  start_daemon
  bria_init
}

teardown_file() {
  stop_daemon
}

@test "Fund an address and see if the balance is reflected" {
  bitcoin-cli -regtest sendtoaddress $(bria new-address -w default --raw) 50

  for i in {1..30}; do
    pending_incoming=$(bria wallet-balance -w default --json | jq -r ".pending_incoming")
    if [[ $pending_incoming == "5000000000" ]]; then success="true"; break; fi;
    sleep 1
  done
  if [[ $success != "true" ]]; then exit 1; fi;
}

@test "Create batch group and have two queued payouts on it" {
  bria create-batch-group --name high
  bria queue-payout --wallet default --group-name high --destination bcrt1q208tuy5rd3kvy8xdpv6yrczg7f3mnlk3lql7ej --amount 200000
  bria queue-payout --wallet default --group-name high --destination bcrt1q3rr02wkkvkwcj7h0nr9dqr9z3z3066pktat7kv --amount 200000

  for i in {1..30}; do
    encumbered_outgoing=$(bria wallet-balance -w default --json | jq -r ".encumbered_outgoing")
    if [[ $encumbered_outgoing == "400000" ]]; then success="true"; break; fi
    sleep 1
  done
  if [[ $success != "true" ]]; then exit 1; fi
}
