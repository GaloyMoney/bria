<!-- omit in toc -->
# Bria
The bridge from your applications to the bitcoin network.

Bria enables transaction batching and UTXO management providing the liquidity of on-chain UTXOs to multiple consumers.

<screenshots>

<details>
<summary>Table of Contents</summary>

- [Key features](#key-features)
- [Demo video](#demo-video)
- [Quickstart](#quickstart)
- [Build from source](#build-from-source)
- [Configuration](#configuration)
- [Usage](#usage)
- [Developing](#developing)

</details>

## Key features
- multi account / multi wallet / multi queue
  - you can configure multiple wallets scoped to an account
  - signing via multiple supported remote signers including feeding PSBTs manually
  - transaction batching via configurable payout queues (check the demo for details)

- cloud ready - intended for use as part of a distributed system
  - designed to be horizontally scalable
  - support for idempotent operations via external IDs
  - embed and update external metadata on addresses and payouts to reference external data
  - globally ordered event sequence can be streamed to achieve guaranteed eventual consistency

- advanced accounting via an embedded ledger
  - internal use of double sided bookkeeping
  - database dump of ledger conforms with accounting best practices
  - great for accountants / auditors to know exactly what is going on

- secure by design
  - extensive automated testing (unit + integration in rust, end-to-end using BATS)
  - all sensitive credentials (like remote signer config) encrypted at rest to prevent db leaks comprimising funds

## Demo video
<a href="https://www.loom.com/share/53e38dc7d1694b11a09b08fc32c584c8">
    <img src="https://cdn.loom.com/sessions/thumbnails/53e38dc7d1694b11a09b08fc32c584c8-1689716086737-with-play.gif" alt="Understanding Bria: Transaction Batching and UTXO Management [🎥]" width="300">
</a>

## Quickstart
* Requirements on Debian / Ubuntu Linux
  ```
  # git, make, direnv
  sudo apt install git make direnv
  # Docker with the Compose plugin is needed to run the preconfigured environment
  # https://docs.docker.com/desktop/install/linux-install/
  # https://docs.docker.com/compose/install/linux/
  ```
* Download the source code
  ```
  git clone https://github.com/GaloyMoney/bria
  cd bria
  ```
* download the compiled release binary
  ```
  # use the latest version from https://github.com/GaloyMoney/bria/releases
  version=0.1.40
  # on linux
  build=unknown-linux-musl
  # on MacOS use:
  # build=apple-darwin

  # download
  wget https://github.com/GaloyMoney/bria/releases/download/${version}/bria-x86_64-${build}-${version}.tar.gz

  # unpack to the ./target/debug directory
  mkdir -p target/debug
  tar -xvzf bria-x86_64-${build}-${version}.tar.gz --strip-components 1 -C target/debug

  # add the location of the binary to the PATH
  PATH="${PATH}:$(pwd)/target/debug"
  ```
* Start the preconfigured dependencies with Docker Compose
  ```
  docker compose up -d integration-deps
  ```
* Provide a database encryption key
  ```
  export SIGNER_ENCRYPTION_KEY="0000000000000000000000000000000000000000000000000000000000000000"
  ```
* Start the bria daemon with the [default configuration](tests/e2e/bria.local.yml)
  ```
  bria daemon -c ./tests/e2e/bria.local.yml postgres://user:password@127.0.0.1:5432/pg run
  ```
* In a new terminal import the functions from the [helpers.bash file](tests/e2e/helpers.bash)
  ```
  source tests/e2e/helpers.bash
  ```
* Bootstrap, create a Bria account and import a wallet
  ```
  bria_init
  ```
* Initialize the local regtest network
  ```
  bitcoind_init
  ```

* For testing further see the commands in the [helpers.bash file](tests/e2e/helpers.bash)

## Build from source

* Install the Rust toolchain
  ```
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  source "$HOME/.cargo/env"
  ```
* download the source code
  ```
  git clone https://github.com/GaloyMoney/bria
  ```
* build
  ```
  cd bria
  make build
  ```
* add the location of the binary to the PATH
  ```
  PATH="${PATH}:$(pwd)/target/debug"
  ```

## Configuration
* Connect to the dependencies following the example in the [docker-compose file](docker-compose.yml)
  - postgres - to store the internal state
  - bitcoind - to provide the chain state to the electrum server
  - fulcrum - a performant electrum server
  - bitcoind-signer - a potential signer
  - lnd - a potential signer
  - mempool - self-hostable backend used for fee estimations
  - otel-agent: optional for observability

* provide the database connection parameters as environment variables
  ```
  # create a .envrc file
  cat <<EOF > .envrc
  export PG_HOST=127.0.0.1
  export PG_CON=postgres://user:password@${PG_HOST}:5432/pg
  EOF

  direnv allow
  ```

## Usage

* provide a database encryption key
  ```
  export SIGNER_ENCRYPTION_KEY="0000000000000000000000000000000000000000000000000000000000000000"
  ```
* start the bria daemon with the [default configuration](tests/e2e/bria.local.yml)
  ```
  bria daemon -c ./tests/e2e/bria.local.yml run
  ```
* create an admin API key (stored in the .bria folder)
  ```
  bria admin bootstrap
  ```
* list accounts
  ```
  bria admin list-accounts
  ```
* create the profile API key (scoped to the account)
  ```
  bria admin create-account -n default
  ```
* create a single-sig wallet for the default bria account
  ```
  bria create-wallet -n default descriptors -d "wpkh([6f2fa1b2/84'/0'/0']tpubDDDDGYiFda8HfJRc2AHFJDxVzzEtBPrKsbh35EaW2UGd5qfzrF2G87ewAgeeRyHEz4iB3kvhAYW1sH6dpLepTkFUzAktumBN8AXeXWE9nd1/0/*)#l6n08zmr" \
      -c "wpkh([6f2fa1b2/84'/0'/0']tpubDDDDGYiFda8HfJRc2AHFJDxVzzEtBPrKsbh35EaW2UGd5qfzrF2G87ewAgeeRyHEz4iB3kvhAYW1sH6dpLepTkFUzAktumBN8AXeXWE9nd1/1/*)#wwkw6htm"
  ```

* For testing further see the commands in the [helpers.bash file](tests/e2e/helpers.bash)

## Developing
For developing all dependencies are run via docker compose

* To run the tests make sure `PG_CON` is pointing to the PG instance inside docker:
  ```
  # create an .envrc file
  cat <<EOF > .envrc
  export PG_HOST=127.0.0.1
  export PG_CON=postgres://user:password@${PG_HOST}:5432/pg
  EOF

  direnv allow
  ```

* Add the dev dependencies
  ```
  make install-dev-deps
  ```

* Run the tests via
  ```
  make reset-deps next-watch
  ```

* For bash based e2e tests we use [bats](https://bats-core.readthedocs.io/en/stable/) as a test runner.
Run the tests via:
  ```
  make e2e
  ```

* If your e2e tests stall and you want to inspect the state (or just want to play around locally) then:
  ```
  make local-daemon
  ```
* Will bring up the daemon and you can run cli commands against it eg:
  ```
  cargo run --bin bria admin list-accounts
  ```
