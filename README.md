# Bria
The bridge from your applications to the bitcoin network.

Bria enables you to share access to onchain bitcoin liquidity in an efficient manner to multiple consumers.

Key features of bria are:
- configuration of multiple wallets
- remote / offline signing
- recording of metadata and external ids (for idempotency) for payouts and addresses
- transaction batching via configurable payout queues
- 'outbox' pattern for informing clients of state changes via a globally ordered event sequence

## Demo

To see bria in action you can watch [this end-to-end demo](https://www.loom.com/share/53e38dc7d1694b11a09b08fc32c584c8?sid=d0008868-ffa0-4915-98b3-ae20f64985b8)

## Developing

For developing all dependencies are run via docker compose

To run the tests make sure `PG_CON` is pointing to the PG instance inside docker:
```
$ cat <<EOF > .envrc
export PG_HOST=127.0.0.1
export PG_CON=postgres://user:password@${PG_HOST}:5432/pg
EOF
direnv allow
```

Add dev dependencies:
```
$ make install-dev-deps
```

Run the tests via:
```
$ make reset-deps next-watch
```

For bash based e2e tests we use [bats](https://bats-core.readthedocs.io/en/stable/) as a test runner.
Run the tests via:
```
$ make e2e
```

If your e2e tests stall and you want to inspect the state (or just want to play around locally) then:
```
$ make local-daemon
```
Will bring up the daemon and you can run cli commands against it eg:
```
$ cargo run --bin bria admin list-accounts
```
