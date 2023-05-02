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