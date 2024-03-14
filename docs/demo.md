## Demo walkthough
* Start the preconfigured dependencies
  ```
  make reset-deps
  ```
* Provide a database encryption key
  ```
  export SIGNER_ENCRYPTION_KEY="0000000000000000000000000000000000000000000000000000000000000000"
  ```
* Add bria to Path
  ```
  export PATH="${PATH}:$(pwd)/target/debug"
  ```
* Start the bria daemon with the [default configuration](../tests/e2e/bria.local.yml) and bootstrap
  ```
  bria daemon --config ./tests/e2e/bria.local.yml postgres://user:password@127.0.0.1:5432/pg dev
  ```

* Create aliases to work with the docker containers
  ```
  alias bitcoin_cli="docker exec bria-bitcoind-1 bitcoin-cli"
  alias bitcoin_signer_cli="docker exec bria-bitcoind-signer-1 bitcoin-cli"
  ```
* Initialize the local bitcoind on regtest
  ```
  bitcoin_cli createwallet "default"
  bitcoin_cli generatetoaddress 200 "$(bitcoin_cli getnewaddress)"
  ```
* Create a bitcoind wallet using a [sample private descriptor](../tests/e2e/bitcoind_signer_descriptors.json)
  ```
  bitcoin_signer_cli createwallet "default"
  bitcoin_signer_cli -rpcwallet=default importdescriptors "$(cat tests/e2e/bitcoind_signer_descriptors.json)"
  ```
* Create a Bria account
  ```
  export PATH="${PATH}:$(pwd)/target/debug"
  bria admin create-account --name default
  ```
* Import the wallet used in the signer bitcoind with it's public descriptor
  ```
  bria create-wallet -n default descriptors -d "wpkh([6f2fa1b2/84'/0'/0']tpubDDDDGYiFda8HfJRc2AHFJDxVzzEtBPrKsbh35EaW2UGd5qfzrF2G87ewAgeeRyHEz4iB3kvhAYW1sH6dpLepTkFUzAktumBN8AXeXWE9nd1/0/*)#l6n08zmr" \
      -c "wpkh([6f2fa1b2/84'/0'/0']tpubDDDDGYiFda8HfJRc2AHFJDxVzzEtBPrKsbh35EaW2UGd5qfzrF2G87ewAgeeRyHEz4iB3kvhAYW1sH6dpLepTkFUzAktumBN8AXeXWE9nd1/1/*)#wwkw6htm"
  ```
* Create an address
  ```
  bria new-address -w default --external-id my-id --metadata '{"hello": "world"}'
  ```
* Send funds to the wallet
  ```
  bitcoin_cli -regtest sendtoaddress bcrt1qntvhlxgk8jh0a48w49f3z9edlwhv52zz3j9kw9 1
  ```
* Create a payout queue
  ```
  bria create-payout-queue -n my-queue --tx-priority next-block --interval-trigger 10
  ```
* Submit payouts
  ```
  bria submit-payout -w default --queue-name my-queue --destination bcrt1qxcpz7ytf3nwlhjay4n04nuz8jyg3hl4ud02t9t --amount 100000
  bria submit-payout -w default --queue-name my-queue --destination bcrt1qxcpz7ytf3nwlhjay4n04nuz8jyg3hl4ud02t9t --amount 150000
  ```
* Check the wallet balance and all events with metadata
  ```
  bria wallet-balance -w default
  bria watch-events --after 0 --one-shot
  ```
* Mine two blocks
  ```
  bitcoin_cli -generate 2
  ```
* Check the wallet balance and all events with metadata
  ```
  bria wallet-balance -w default
  bria watch-events --after 0 --one-shot
  ```
* Sign
  ```
  bria set-signer-config \
    --xpub "68bfb290" bitcoind \
    --endpoint "localhost:18543" \
    --rpc-user "rpcuser" \
    --rpc-password "rpcpassword"
  ```
* Mine two blocks
  ```
  bitcoin_cli -generate 2
  ```
* Check the wallet balance with now completed payouts
  ```
  bria wallet-balance -w default
  ```
* Explore more options with:
  ```
  bria help
  bria <COMMAND> help
  ```
