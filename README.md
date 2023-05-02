# Bria
The bridge from your applications to the bitcoin network

## e2e demo
In tab 1 (server)
```
make build
export PATH="${PATH}:$(pwd)/target/debug"
make reset-deps
bria daemon --config ./tests/e2e/bria.local.yml
```

In tab 2 (cli)
```
# setup
export PATH="${PATH}:$(pwd)/target/debug"
source ./tests/e2e/helpers.bash
bitcoind_init

# Bootstrapping
bria admin bootstrap
cat .bria/admin-api-key

# Create account
bria admin create-account -n demo
cat .bria/profile-api-key
bria admin list-accounts

# See the default profile (created during account creation)
bria list-profiles

# Create wallet
bria import-xpub -n lnd_key -x tpubDD4vFnWuTMEcZiaaZPgvzeGyMzWe6qHW8gALk5Md9kutDvtdDjYFwzauEFFRHgov8pAwup5jX88j5YFyiACsPf3pqn5hBjvuTLRAseaJ6b4 -d m/84h/0h/0h
bria create-wallet -n demo -x lnd_key

# Create address

bria new-address -w demo
bria list-addresses -w demo
bria_addr="$(bria new-address -w demo -m '{"hello": "world"}' | jq -r 'address')"
bria list-addresses -w demo

# Watching the blockchain for money in / out
bria wallet-balance -w demo
bitcoin_cli -regtest sendtoaddress ${bria_addr} 1
# see money come in via pending
bria wallet-balance -w demo

# send money to externally created address
lnd_cli newaddress p2wkh | jq -r '.address'
lnd_cli newaddress p2wkh | jq -r '.address'
lnd_addr="$(lnd_cli newaddress p2wkh | jq -r '.address')" # <- this should be a new address

bria list-addresses -w demo # <- 2 adderesses visible
bitcoin_cli -regtest sendtoaddress ${lnd_addr} 1
bria wallet-balance -w demo
bria list-addresses -w demo # <- 3 adderesses visible

# Settle income
bitcoin_cli -generate 2
bria wallet-balance -w demo

# send money out (extenally)
bitcoind_addr="$(bitcoin_cli -regtest getnewaddress)"
lnd_cli sendcoins --addr=${bitcoind_addr} --amt=50000000
bria wallet-balance -w demo
bitcoin_cli -generate 1
bria wallet-balance -w demo

# sweep wallet to reset balances
lnd_cli sendcoins --addr=${bitcoind_addr} --sweepall

# Send money from bria
bria queue-payout -h
bria create-batch-group -h

bria_cmd create-batch-group --name demo --interval-trigger 5
bria_cmd queue-payout --wallet demo --group-name demo --destination bcrt1q208tuy5rd3kvy8xdpv6yrczg7f3mnlk3lql7ej --amount 75000000
bria wallet-balance -w demo
bria list-payouts -w demo

bitcoin_cli -regtest sendtoaddress ${bria_addr} 1
bria wallet-balance -w demo
bria list-payouts -w demo # batch_id = null
bitcoin_cli -generate 2
bria wallet-balance -w demo
bria list-payouts -w demo # batch_id != null

# signing
batch_id=$(bria list-payouts -w demo | jq -r '.payouts[0].batchId')
bria list-signing-sessions -b "${batch_id}"

bria set-signer-config --xpub lnd_key lnd --endpoint "${LND_ENDPOINT}" --macaroon-file "./dev/lnd/regtest/lnd.admin.macaroon" --cert-file "./dev/lnd/tls.cert"
bria list-signing-sessions -b "${batch_id}" # state = Complete
bitcoin_cli -generate 2
bria wallet-balance -w demo
```

In tab 3 (events)
```
export PATH="${PATH}:$(pwd)/target/debug"
bria watch-events
```
Back in tab 2
```
bitcoin_cli -regtest sendtoaddress ${bria_addr} 1
bitcoin_cli -generate 2
bria watch-events -a 7
bria watch-events -a 0
```
