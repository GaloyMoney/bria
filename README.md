<!-- omit in toc -->
# Bria
The bridge from your applications to the bitcoin network.

Bria enables transaction batching and UTXO management providing the liquidity of on-chain UTXOs to multiple consumers.

<details>
<summary>Table of Contents</summary>

- [Key features](#key-features)
- [Developing](#developing)
  - [Dependencies](#dependencies)
    - [Nix package manager](#nix-package-manager)
    - [direnv \>= 2.30.0](#direnv--2300)
    - [Docker](#docker)
  - [Demo walkthrough](#demo-walkthrough)
  - [Testing](#testing)
    - [Running tests](#running-tests)
    - [End-to-end tests](#end-to-end-tests)
    - [Local daemon for E2E tests and exploration](#local-daemon-for-e2e-tests-and-exploration)
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

- Payjoin support for UTXO consolidation and transaction efficiency

## Developing

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

### Demo walkthrough

For a step-by-step guide on how to get started with the demo, see the [demo walkthrough](docs/demo.md).

### Testing

To run commands in the [Nix](https://github.com/DeterminateSystems/nix-installer) environment, there are two primary methods:

1. **Using `direnv`:** If `direnv` is installed and hooked into your shell, simply `cd` into the repository. Nix will automatically bootstrap the environment for you using the flake. On the first run, you'll need to execute `direnv allow` to load the environment configuration.

2. **Manual entry:** Alternatively, you can manually enter the environment by executing `nix develop`. You can also run a specific command directly with `nix develop --command <command>`, or use the environment as you prefer.

#### Running tests

- to run the tests, use the following command:
    ```bash
    make reset-deps next-watch
    ```

#### End-to-end tests

- for bash-based end-to-end tests, we use [bats](https://bats-core.readthedocs.io/en/stable/) as a test runner. To execute these tests, run:
    ```bash
    make e2e
    ```

#### Local daemon for E2E tests and exploration

- if your end-to-end tests stall, or if you simply wish to inspect the state or experiment locally, you can start the local daemon with:
    ```bash
    make local-daemon
    ```
- once the daemon is up, you can run CLI commands against it. For example:
    ```bash
    cargo run --bin bria help
    ```

## License
[Mozilla Public License 2.0](LICENSE)
