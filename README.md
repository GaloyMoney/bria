<!-- omit in toc -->
# Bria
The bridge from your applications to the bitcoin network.

Bria enables transaction batching and UTXO management providing the liquidity of on-chain UTXOs to multiple consumers.

<details>
<summary>Table of Contents</summary>

- [Key features](#key-features)
- [Quickstart](#quickstart)
  - [Dependencies](#dependencies)
    - [Nix package manager](#nix-package-manager)
    - [direnv \>= 2.30.0](#direnv--2300)
    - [Docker](#docker)
  - [Build from source](#build-from-source)
  - [Demo Walkthrough](#demo-walkthrough)
- [Developing with Nix Environment](#developing-with-nix-environment)
  - [Running Tests](#running-tests)
  - [End-to-End Tests](#end-to-end-tests)
  - [Local Daemon for E2E Tests and Exploration](#local-daemon-for-e2e-tests-and-exploration)
- [License](#license)

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

## Quickstart

### Dependencies

#### Nix package manager
* recommended install method using https://github.com/DeterminateSystems/nix-installer
  ```
  curl --proto '=https' --tlsv1.2 -sSf -L https://install.determinate.systems/nix | sh -s -- install
  ```

#### direnv >= 2.30.0
* recommended install method from https://direnv.net/docs/installation.html:
  ```
  curl -sfL https://direnv.net/install.sh | bash
  echo "eval \"\$(direnv hook bash)\"" >> ~/.bashrc
  source ~/.bashrc
  ```

#### Docker
* choose the install method for your system https://docs.docker.com/desktop/


### Build from source
* Download the source code
  ```
  git clone https://github.com/GaloyMoney/bria
  ```

* Build
  ```
  cd bria
  direnv allow
  make build
  ```
* Add the location of the binary to the PATH
  ```
  PATH="${PATH}:$(pwd)/target/debug"
  ```
### [Demo Walkthrough](docs/demo.md)

## Developing with Nix Environment

To run commands in the [Nix](https://github.com/DeterminateSystems/nix-installer) environment, there are two primary methods:

1. **Using `direnv`:** If `direnv` is installed and hooked into your shell, simply `cd` into the repository. Nix will automatically bootstrap the environment for you using the flake. On the first run, you'll need to execute `direnv allow` to load the environment configuration.

2. **Manual Entry:** Alternatively, you can manually enter the environment by executing `nix develop`. You can also run a specific command directly with `nix develop --command <command>`, or use the environment as you prefer.

### Running Tests

- To run the tests, use the following command:
    ```bash
    make reset-deps next-watch
    ```

### End-to-End Tests

- For bash-based end-to-end tests, we use [bats](https://bats-core.readthedocs.io/en/stable/) as a test runner. To execute these tests, run:
    ```bash
    make e2e
    ```

### Local Daemon for E2E Tests and Exploration

- If your end-to-end tests stall, or if you simply wish to inspect the state or experiment locally, you can start the local daemon with:
    ```bash
    make local-daemon
    ```
- Once the daemon is up, you can run CLI commands against it. For example:
    ```bash
    cargo run --bin bria help
    ```

## License
[Mozilla Public License 2.0](LICENSE)
