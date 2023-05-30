# [bria release v0.1.10](https://github.com/GaloyMoney/bria/releases/tag/0.1.10)


### Miscellaneous Tasks

- Add BITCOIND_SIGNER_ENDPOINT to dev daemon
- Add name to PayoutQueueNotFound

# [bria release v0.1.9](https://github.com/GaloyMoney/bria/releases/tag/0.1.9)


### Miscellaneous Tasks

- Bump dependencies (#197)

# [bria release v0.1.8](https://github.com/GaloyMoney/bria/releases/tag/0.1.8)


### Miscellaneous Tasks

- Create dev queue in dev-bootstrap

# [bria release v0.1.7](https://github.com/GaloyMoney/bria/releases/tag/0.1.7)


### Miscellaneous Tasks

- Include proportional fees in outbox (#196)
- Replace mempool_space with mempool_space_client (#195)

# [bria release v0.1.6](https://github.com/GaloyMoney/bria/releases/tag/0.1.6)


### Miscellaneous Tasks

- Pass only hostname for mempool_space (#194)
- Update sqlx-data
- Make index incrementation atomic

# [bria release v0.1.5](https://github.com/GaloyMoney/bria/releases/tag/0.1.5)


### Bug Fixes

- Typo proporional -> proportional (#193)
- Clippy

### Features

- Estimate_fee (#192)
- Add mempool-space to docker-compose (#186)

### Miscellaneous Tasks

- Rename mempool in docker compose
- Half hour priority
- Bump prost-wkt-types to released version
- Bump base64 from 0.21.0 to 0.21.1 (#189)
- Bump electrum timeout to 60s
- Remove SyncProgress (not supported for electrum)

### Refactor

- Add url field to MempoolSpaceClient (#191)

### Testing

- Restart mempool in e2e tests
- Increase timeout for bitcoind_sync

# [bria release v0.1.4](https://github.com/GaloyMoney/bria/releases/tag/0.1.4)


### Refactor

- Use better batching for bdk persistance (#188)

# [bria release v0.1.3](https://github.com/GaloyMoney/bria/releases/tag/0.1.3)


### Miscellaneous Tasks

- Pass progress tracker to bdk sync
- Set max_retry_delay to 60s for sync_wallet
- Dedup payout_queue scheduling
- Bump reqwest from 0.11.17 to 0.11.18 (#187)

# [bria release v0.1.2](https://github.com/GaloyMoney/bria/releases/tag/0.1.2)


### Miscellaneous Tasks

- Output has_more in span

# [bria release v0.1.1](https://github.com/GaloyMoney/bria/releases/tag/0.1.1)


### Bug Fixes

- Change bitcoin to mainnet for BlockchainConfig (#185)
- Better encumbered fees estimation

### Miscellaneous Tasks

- Sync 100 txs at a time

# [bria release v0.1.0](https://github.com/GaloyMoney/bria/releases/tag/0.1.0)


### Bug Fixes

- Cleanup to the encrypt signer config pr (#184)
- No default for PG_CON

### Features

- [**breaking**] Use meaningful id's for accounts (#181)
- Encrypt and persist signer config (#177)

### Miscellaneous Tasks

- Bump uuid from 1.3.2 to 1.3.3 (#183)
- Handle max_retry_backof and extend signing job
- Remove comment (#179)

### Refactor

- Pass JobsConfig to jobs runner
- UnbatchedPayout.commit_to_batch
- UnbatchedPayouts container
- Extract construct_psbt to PsbtBuilder
- Group_name -> payout_queue

### Testing

- Remove redundant debug output
- Attempt retries in bria_init
- Fix bria.yml for e2e tests

# [bria release v0.0.24](https://github.com/GaloyMoney/bria/releases/tag/0.0.24)


### Miscellaneous Tasks

- Improve tracing
- Bump serde from 1.0.162 to 1.0.163 (#178)
- Bump tokio from 1.28.0 to 1.28.1 (#176)

# [bria release v0.0.23](https://github.com/GaloyMoney/bria/releases/tag/0.0.23)


### Miscellaneous Tasks

- Add batch_id to payout events
- Remove redundant config options (#175)

# [bria release v0.0.22](https://github.com/GaloyMoney/bria/releases/tag/0.0.22)


### Refactor

- CommittedToBatch

# [bria release v0.0.21](https://github.com/GaloyMoney/bria/releases/tag/0.0.21)


### Features

- Add update-batch-group (#167)
- Payout events (#168)

### Miscellaneous Tasks

- Add optional wallet creation in dev bootstrap (#173)
- Missing payout events (#172)
- Rename batch-group (#170)

### Refactor

- Rename payout-queued -> payout-submitted (#174)
- Fix outstanding naming in job/mod.rs
- Logical -> effective (#169)

# [bria release v0.0.20](https://github.com/GaloyMoney/bria/releases/tag/0.0.20)


### Bug Fixes

- Batch group description is optional

### Miscellaneous Tasks

- Add description in list-batch-groups (#165)
- Add DbConfig (#164)
- Add electrum fee estimator

### Refactor

- Multi change outputs (#166)

# [bria release v0.0.19](https://github.com/GaloyMoney/bria/releases/tag/0.0.19)


### Features

- --dev flag for daemon to auto bootstrap (#157)
- List-xpubs (#162)

### Miscellaneous Tasks

- Add descriptors to ensure no duplicate use (#163)
- Bump sqlx-ledger
- Bump serde from 1.0.160 to 1.0.162 (#160)

### Testing

- Assert_summaries_match helper

# [bria release v0.0.18](https://github.com/GaloyMoney/bria/releases/tag/0.0.18)


### Bug Fixes

- Do not unwrap derivation path after parse

# [bria release v0.0.17](https://github.com/GaloyMoney/bria/releases/tag/0.0.17)


### Miscellaneous Tasks

- Whitespace
- Whitespace
- Refactor keychain (#161)

# [bria release v0.0.16](https://github.com/GaloyMoney/bria/releases/tag/0.0.16)


### Bug Fixes

- Address pr reviews
- Fix formatting errors
- Add suggested changes

### Features

- Add account balance summary (#159)
- Add import-descriptors (#158)
- Add list-wallets
- Add list_batch_groups

### Miscellaneous Tasks

- Bump clap from 4.2.5 to 4.2.7
- Fix txPriority output in list-batch-groups
- Remove (unused) dust config

# [bria release v0.0.16](https://github.com/GaloyMoney/bria/releases/tag/0.0.16)


### Bug Fixes

- Address pr reviews
- Fix formatting errors
- Add suggested changes

### Features

- Add account balance summary (#159)
- Add import-descriptors (#158)
- Add list-wallets
- Add list_batch_groups

### Miscellaneous Tasks

- Bump clap from 4.2.5 to 4.2.7
- Fix txPriority output in list-batch-groups
- Remove (unused) dust config

# [bria release v0.0.15](https://github.com/GaloyMoney/bria/releases/tag/0.0.15)


### Bug Fixes

- Switch to 'STANDARD' base64 engine to keep trailing '='
- Add 'sighash_type' argument for signing psbt with bitcoind
- Add missing Bitcoind connect branch

### Documentation

- Add demo in readme

### Features

- Update address
- Implement bitcoind signer
- Add 'BitcoindSignerConfig' handling to api grpc interface

### Miscellaneous Tasks

- Poll augmenter from OutboxListener
- Use EcdsaSighashType as const
- Bump serde_with from 2.3.3 to 3.0.0
- Wire augment option through
- Swap out deprecated base64 encode/decode
- Reformat long command
- Bump anyhow from 1.0.70 to 1.0.71

### Refactor

- Declare and assign a DEFAULT_SIGHASH_TYPE for re-use
- Pass sighash from unsigned psbt into signer
- Make OutboxEvent generic
- Add 2nd bitcoind container for signer wallet
- Remove intermediate SignerConfig type in cli module
- Rename confirmed -> settledUtxo in balance summary

### Testing

- Back to bc
- Remove dependency on bc
- Connect to bitcoind-signer for payouts test
- Test update-address via event augmentation
- Restart bitcoind-signer as well to clear chain state
- Flaky lnd_sync tests on re-run, or run after bitcoind_sync
- Add a check for deliberate transition before block mine
- Why sweepall is not confirming in bria
- Add 'exit 1' to some checks
- Swap out sendtoaddress for manual tx to spend unconfirmed
- Add bitcoind sync tests
- Update 'signing-complete' check
- Swap in bitcoind as payout signer

# [bria release v0.0.14](https://github.com/GaloyMoney/bria/releases/tag/0.0.14)


### Miscellaneous Tasks

- Switch event proto idx
- Logical balance before utxos
- Consistent dir namings (daemon-pid)
- Bump sqlx-ledger to v0.7.7
- Bump uuid from 1.3.1 to 1.3.2

# [bria release v0.0.13](https://github.com/GaloyMoney/bria/releases/tag/0.0.13)


### Features

- Watch-events cli cmd

### Miscellaneous Tasks

- Remove redundant FOR UPDATE
- Complete include PayoutInfo in batch metadata
- Remove bria_batch_spent_utxos and revamp utxo handling
- Add involved_keychains to WalletSummary
- Add payout to batch metadata WIP
- Forgot CONFIRMED_UTXO -> SETTLED_UTXO renaming
- Return correct type for event stream
- Add OutboxListener
- Outbox listener WIP
- Persist journal events in outbox
- OutboxEvent boilerplate
- Add account_id to all metadata
- More Outbox boilerplate
- Bump tracing from 0.1.37 to 0.1.38
- Journal events boilerplate
- Cargo update
- Bump sqlx-ledger
- Handle_outbox boilerplate
- Add keep_alive thread to job executor
- Bump serde_json from 1.0.95 to 1.0.96

### Refactor

- Rename entry-types
- BatchInfo -> BatchWalletInfo
- Remove additional_metadata from PayoutQueuedMeta
- Wallet_summary.signing_keychains
- Remove Uuid from batch/repo.rs
- WalletTransactionSummary naming
- Move PayoutDestination to primitives
- Consistent column naming
- Better naming for templates
- Small cleanups

# [bria release v0.0.12](https://github.com/GaloyMoney/bria/releases/tag/0.0.12)


### Bug Fixes

- Set address events

### Refactor

- Proper signing session initialization event
- Proper address initialization event
- Proper xpub initialization event
- Move original out of XPubValue
- Payouts as events
- Events in batch_group
- Better wallet structure
- Persist wallet with events
- Use EntityEvents::persist in signing session repo
- Use EntityEvents::persist
- Use events in xpub entity

# [bria release v0.0.11](https://github.com/GaloyMoney/bria/releases/tag/0.0.11)


### Bug Fixes

- Clippy
- NewUtxo field visibility

### Features

- List-addresses cli cmd
- Add addresses repository
- Pass metadata json arg in to grpc service
- Add 'metadata' arg to queue-payout cmd

### Miscellaneous Tasks

- Sync addresses in sync_wallet job
- Bump tracing-subscriber from 0.3.16 to 0.3.17
- Improve address entity
- Submit batch execution
- Update 'h2' for RUSTSEC-2023-0034 vulnerability
- Implement Display trait on AddressCreationInfo
- Submit_batch template
- Use tx_summary in create_batch template
- Bump clap from 4.2.2 to 4.2.4
- Signing finalized / broadcasting broadcasts
- Bump tonic-build from 0.9.1 to 0.9.2
- Some pre-accounting cleanup
- Batch_finalizing
- Set-signer-config triggers batch-signing
- Batch_signing
- Bump tonic-build from 0.9.1 to 0.9.2
- List-signing-sessions cli cmd
- List-signing-sessions
- Persist updated sessions
- Complete persistance of new signing sessions
- Some signing boilerplate
- Move jobs to singular
- Add signing_session module
- Pass XPubs to jobs
- Introduce entity module
- Access xpubs via wallet
- Add bitcoind/signet.conf
- Bump prost from 0.11.8 to 0.11.9
- Use forked prost-wkt-types
- Improve rust idioms
- Handle json conversion error in ApiClient::queue_payout
- Handle struct parsing error in Bria::queue_payout
- Add prost-types

### Refactor

- Make external_id is address by default
- Persist address via events
- Persist_new_session -> persist_sessions
- Assign address_id to external_id if none is passed in
- Make (address_string, keychain_id) combination unique
- Add 'profile_id' to Address entity
- Change 'new_external_address' return to domain AddressCreationInfo type
- Add new props to NewAddress grpc request
- Add new props to new-address cli command
- Pass in pg tx to utxo use cases
- Restructure foreign references
- Make queue_payout metadata prop optional

### Testing

- Add list-addresses to e2e tests
- Add new args to new-address test
- Add metadata arg to queue-payout test

# [bria release v0.0.10](https://github.com/GaloyMoney/bria/releases/tag/0.0.10)


### Bug Fixes

- Check-code
- Handle spent change utxo
- Correct deferred logical out

### Miscellaneous Tasks

- Sync tx confirmation in line
- Bump tonic from 0.9.1 to 0.9.2
- Bump clap from 4.2.1 to 4.2.2

# [bria release v0.0.9](https://github.com/GaloyMoney/bria/releases/tag/0.0.9)


### Miscellaneous Tasks

- Return error on ElectrumBlockchain config

# [bria release v0.0.8](https://github.com/GaloyMoney/bria/releases/tag/0.0.8)


### Bug Fixes

- Support for vpub import

# [bria release v0.0.7](https://github.com/GaloyMoney/bria/releases/tag/0.0.7)


### Bug Fixes

- Missing commit call
- Only auth with active keys

### Features

- Introduce profile

### Miscellaneous Tasks

- Expose create profile api key

### Refactor

- Rename account -> profile in token_store

# [bria release v0.0.6](https://github.com/GaloyMoney/bria/releases/tag/0.0.6)


### Features

- List accounts

### Miscellaneous Tasks

- Rename AccountCreate -> CreateAccount

# [bria release v0.0.5](https://github.com/GaloyMoney/bria/releases/tag/0.0.5)


### Bug Fixes

- Bria home in release images

### Miscellaneous Tasks

- Bump sqlx-ledger from 0.5.11 to 0.5.12

# [bria release v0.0.4](https://github.com/GaloyMoney/bria/releases/tag/0.0.4)


### Bug Fixes

- Release images

# [bria release v0.0.3](https://github.com/GaloyMoney/bria/releases/tag/0.0.3)


### Bug Fixes

- Dev version

# [bria release v0.0.2](https://github.com/GaloyMoney/bria/releases/tag/0.0.2)
