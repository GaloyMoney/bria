<!-- omit in toc -->
# Bria
The bridge from your applications to the bitcoin network.

Bria enables transaction batching and UTXO management providing the liquidity of on-chain UTXOs to multiple consumers.

<screenshots>

<details>
<summary>Table of Contents</summary>

- [Key features](#key-features)
- [Demo](#demo)
- [Installation](#installation)
- [Configuration](#configuration)
- [Usage](#usage)
- [Developing](#developing)

</details>

## Key features
- configuration and management of multiple wallets
  - all wallets are scoped within an account

- remote / offline signing
    - no hot wallet in Bria (for now)
    - multiple external signers are supported including feeding PSBTs manually

- recording of metadata and external ids (for idempotency) for payouts and addresses
  - made to be part of a distributed system

- 'outbox' pattern for informing clients of state changes via a globally ordered event sequence
  - to keep upstream clients in sync with the internal state
  - safely to recover from disconnections

- double sided bookkeeping by an embedded ledger that conforms with accounting standards

- transaction batching via configurable payout queues
  - details in the demo

- extensive automated testing
  - unit and integration tests in Rust
  - end to end tests using BATS

## Demo
[![Watch the video](https://github.com/yourusername/repository/blob/path/to/animation.gif)](https://www.loom.com/share/53e38dc7d1694b11a09b08fc32c584c8)


## Installation
* Requirements on Debian / Ubuntu Linux
    ```
    # git, make, direnv
    sudo apt install git make direnv
    ```
* download the release binary from https://github.com/GaloyMoney/bria/releases
    ```
    version=0.1.40
    # on linux
    build=unknown-linux-musl # on MacOS use: build=apple-darwin

    wget https://github.com/GaloyMoney/bria/releases/download/${version}/bria-x86_64-unknown-linux-musl-${version}.tar.gz
    ```

* build from source
    ```
    # install the Rust toolchain
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
    source "$HOME/.cargo/env"

    # download the source code
    git clone https://github.com/GaloyMoney/bria

    # build
    cd bria
    make build

    # add the location of the binary to the PATH
    PATH="${PATH}:$(pwd)/target/debug"
    ```

## Configuration
* Connect to dependencies as found as an example in the [docker-compose file](docker-compose.yml)
  - postgres - to store the internal state
  - bitcoind - to provide the chain state to the electrum server
  - fulcrum - a performant electrum server
  - bitcoind-signer - a potential signer
  - lnd - a potential signer
  - mempool - self-hostable backend used for fee estimations
  - otel-agent: optional for observability

* For testing start the preconfigured dependencies with docker
    ```
    docker compose up -d integration-deps
    ```
* provide the connection parameters as environment variables

    ```
    # create the .envrc file
    cat <<EOF > .envrc
    export PG_HOST=127.0.0.1
    export PG_CON=postgres://user:password@${PG_HOST}:5432/pg
    EOF

    direnv allow
    ```

## Usage

* provide a database encryption key
  ```
  export SIGNER_ENCRYPTION_KEY="00000000000000000000"
  ```

* start the bria daemon with the [default configuration](tests/e2e/bria.local.yml)
  ```
  bria daemon -c .tests/e2e/bria.local.yml run
  ```
* create an API key (stored in the .bria folder)
bria admin bootstrap

* list accounts
  ```
  bria admin list-accounts
  ```

For testing see the commands in the [helpers.bash file](tests/e2e/helpers.bash)


## Developing
For developing all dependencies are run via docker compose

* To run the tests make sure `PG_CON` is pointing to the PG instance inside docker:
  ```
  # create the .envrc file
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
