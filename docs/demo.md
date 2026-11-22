## Demo walkthrough

> **Local regtest demo only.** The Docker configuration, example RPC credentials,
> signer keys, descriptors, and addresses in this guide are for disposable test
> environments. Never reuse them on a network holding real funds.

### Build from source
* clone the bria repository
  ```
  git clone https://github.com/GaloyMoney/bria
  ```
* go to the bria directory and run direnv
  ```
  cd bria && direnv allow
  ```
* build the bria binary
  ```
  make build
  ```

### Walkthrough steps
* start the preconfigured dependencies
  ```
  make reset-deps
  ```
* add bria to PATH
  ```
  export PATH="${PATH}:$(pwd)/target/debug"
  ```
* start the Bria daemon in development mode with the [regtest configuration](../bats/bria.local.yml) and bootstrap.
  The `dev` subcommand uses a built-in signer-encryption key for local testing;
  `SIGNER_ENCRYPTION_KEY` does not need to be exported for this command.
  ```
  bria daemon --config ./bats/bria.local.yml postgres://user:password@127.0.0.1:5432/pg dev
  ```
* leave the daemon running. In a second terminal, return to the cloned repository
  directory, load the development environment, and put the binary on `PATH`:
  ```
  cd bria
  direnv allow
  export PATH="${PATH}:$(pwd)/target/debug"
  ```

* create aliases to work with the docker containers
  ```
  alias bitcoin_cli="docker exec bria-bitcoind-1 bitcoin-cli"
  alias bitcoin_signer_cli="docker exec bria-bitcoind-signer-1 bitcoin-cli"
  ```
* verify that **both** Bitcoin containers are on regtest before creating wallets
  or sending test coins. Both responses must contain `"chain": "regtest"`;
  stop if either reports another network:
  ```
  bitcoin_cli getblockchaininfo
  bitcoin_signer_cli getblockchaininfo
  ```
* initialize the local bitcoind on regtest
  ```
  bitcoin_cli createwallet "default"
  bitcoin_cli generatetoaddress 200 "$(bitcoin_cli getnewaddress)"
  ```
* create a bitcoind wallet using a [sample private descriptor](../bats/bitcoind_signer_descriptors.json)
  ```
  bitcoin_signer_cli createwallet "default"
  bitcoin_signer_cli -rpcwallet=default importdescriptors "$(cat bats/bitcoind_signer_descriptors.json)"
  ```
* create a Bria account
  ```
  export PATH="${PATH}:$(pwd)/target/debug"
  bria admin create-account --name default
  ```
* import the wallet used in the signer bitcoind with its public descriptor
  ```
  bria create-wallet -n default descriptors -d "wpkh([6f2fa1b2/84'/0'/0']tpubDDDDGYiFda8HfJRc2AHFJDxVzzEtBPrKsbh35EaW2UGd5qfzrF2G87ewAgeeRyHEz4iB3kvhAYW1sH6dpLepTkFUzAktumBN8AXeXWE9nd1/0/*)#l6n08zmr" \
      -c "wpkh([6f2fa1b2/84'/0'/0']tpubDDDDGYiFda8HfJRc2AHFJDxVzzEtBPrKsbh35EaW2UGd5qfzrF2G87ewAgeeRyHEz4iB3kvhAYW1sH6dpLepTkFUzAktumBN8AXeXWE9nd1/1/*)#wwkw6htm"
  ```
* create an address
  ```
  bria new-address -w default --external-id my-id --metadata '{"hello": "world"}'
  ```
* send funds to the wallet
  ```
  bitcoin_cli -regtest sendtoaddress bcrt1qntvhlxgk8jh0a48w49f3z9edlwhv52zz3j9kw9 1
  ```
* create a payout queue
  ```
  bria create-payout-queue -n my-queue --tx-priority next-block --interval-trigger 10
  ```
* submit payouts
  ```
  bria submit-payout -w default --queue-name my-queue --destination bcrt1qxcpz7ytf3nwlhjay4n04nuz8jyg3hl4ud02t9t --amount 100000
  bria submit-payout -w default --queue-name my-queue --destination bcrt1qxcpz7ytf3nwlhjay4n04nuz8jyg3hl4ud02t9t --amount 150000
  ```
* check the wallet balance and all events with metadata
  ```
  bria wallet-balance -w default
  bria watch-events --after 0 --one-shot
  ```
* mine two blocks
  ```
  bitcoin_cli -generate 2
  ```
* check the wallet balance and all events with metadata
  ```
  bria wallet-balance -w default
  bria watch-events --after 0 --one-shot
  ```
* sign
  ```
  bria set-signer-config \
    --xpub "68bfb290" bitcoind \
    --endpoint "localhost:18543" \
    --rpc-user "rpcuser" \
    --rpc-password "rpcpassword"
  ```
* mine two blocks
  ```
  bitcoin_cli -generate 2
  ```
* check the wallet balance with now completed payouts
  ```
  bria wallet-balance -w default
  ```
* explore more options with:
  ```
  bria help
  bria <COMMAND> help
  ```
