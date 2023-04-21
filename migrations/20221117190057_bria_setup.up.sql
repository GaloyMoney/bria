CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TYPE KeychainKind AS ENUM ('external', 'internal');

CREATE TABLE bria_admin_api_keys (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  name VARCHAR UNIQUE NOT NULL,
  encrypted_key VARCHAR NOT NULL,
  active BOOL NOT NULL DEFAULT true,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  modified_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE bria_accounts (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  journal_id UUID NOT NULL,
  name VARCHAR UNIQUE NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  modified_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE bria_profiles (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  account_id UUID REFERENCES bria_accounts(id) NOT NULL,
  name VARCHAR NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  modified_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE(account_id, name)
);

CREATE TABLE bria_profile_api_keys (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  profile_id UUID REFERENCES bria_profiles(id) NOT NULL,
  encrypted_key VARCHAR NOT NULL,
  active BOOL NOT NULL DEFAULT true,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  modified_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE bria_xpubs (
  account_id UUID REFERENCES bria_accounts(id) NOT NULL,
  name VARCHAR NOT NULL,
  fingerprint BYTEA NOT NULL,
  parent_fingerprint BYTEA NOT NULL,
  original VARCHAR NOT NULL,
  xpub BYTEA NOT NULL,
  derivation_path VARCHAR,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  modified_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE(account_id, fingerprint),
  UNIQUE(account_id, name)
);

CREATE TABLE bria_xpub_signers (
  id UUID PRIMARY KEY NOT NULL,
  version INT NOT NULL DEFAULT 1,
  account_id UUID REFERENCES bria_accounts(id) NOT NULL,
  xpub_fingerprint BYTEA NOT NULL,
  signer_cfg JSONB NOT NULL,
  FOREIGN KEY (account_id, xpub_fingerprint) REFERENCES bria_xpubs (account_id, fingerprint),
  UNIQUE(account_id, xpub_fingerprint, version)
);

CREATE TABLE bria_wallets (
  id UUID NOT NULL DEFAULT gen_random_uuid(),
  version INT NOT NULL DEFAULT 1,
  account_id UUID REFERENCES bria_accounts(id) NOT NULL,
  onchain_incoming_ledger_account_id UUID NOT NULL,
  onchain_at_rest_ledger_account_id UUID NOT NULL,
  onchain_outgoing_ledger_account_id UUID NOT NULL,
  onchain_fee_ledger_account_id UUID NOT NULL,
  logical_incoming_ledger_account_id UUID NOT NULL,
  logical_at_rest_ledger_account_id UUID NOT NULL,
  logical_outgoing_ledger_account_id UUID NOT NULL,
  dust_ledger_account_id UUID NOT NULL,
  name VARCHAR NOT NULL,
  wallet_cfg JSONB NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  modified_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE(id, version),
  UNIQUE(account_id, name, version)
);

CREATE TABLE bria_utxos (
    idx SERIAL PRIMARY KEY,
    wallet_id UUID NOT NULL,
    keychain_id UUID NOT NULL,
    tx_id VARCHAR NOT NULL,
    vout INTEGER NOT NULL,
    sats_per_vbyte_when_created REAL NOT NULL,
    self_pay BOOLEAN NOT NULL,
    kind KeychainKind NOT NULL,
    address_idx INTEGER NOT NULL,
    value NUMERIC NOT NULL,
    address VARCHAR NOT NULL,
    script_hex VARCHAR NOT NULL,
    bdk_spent BOOLEAN NOT NULL DEFAULT FALSE,
    pending_income_ledger_tx_id UUID NOT NULL,
    confirmed_income_ledger_tx_id UUID DEFAULT NULL,
    spending_batch_id UUID DEFAULT NULL,
    pending_spend_ledger_tx_id UUID DEFAULT NULL,
    confirmed_spend_ledger_tx_id UUID DEFAULT NULL,
    block_height INTEGER DEFAULT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    modified_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(keychain_id, tx_id, vout)
);


CREATE TABLE bria_wallet_keychains (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  wallet_id UUID NOT NULL,
  account_id UUID REFERENCES bria_accounts(id) NOT NULL,
  sequence INT NOT NULL DEFAULT 0,
  keychain_cfg JSONB NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  modified_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE(wallet_id, sequence)
);

CREATE TABLE bria_batch_groups (
  id UUID NOT NULL DEFAULT gen_random_uuid(),
  version INT NOT NULL DEFAULT 1,
  account_id UUID REFERENCES bria_accounts(id) NOT NULL,
  name VARCHAR NOT NULL,
  description VARCHAR,
  batch_cfg JSONB NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  modified_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE(id, version),
  UNIQUE(account_id, name, version)
);

CREATE TABLE bria_payouts (
  id UUID NOT NULL DEFAULT gen_random_uuid(),
  version INT NOT NULL DEFAULT 1,
  account_id UUID REFERENCES bria_accounts(id) NOT NULL,
  wallet_id UUID NOT NULL,
  batch_group_id UUID NOT NULL,
  destination_data JSONB NOT NULL,
  satoshis BIGINT NOT NULL,
  priority INT NOT NULL DEFAULT 100,
  external_id VARCHAR NOT NULL,
  metadata JSONB,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  modified_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE(id, version),
  UNIQUE(account_id, external_id, version)
);

CREATE TABLE bria_batches (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  account_id UUID REFERENCES bria_accounts(id) NOT NULL,
  batch_group_id UUID NOT NULL,
  total_fee_sats BIGINT NOT NULL,
  bitcoin_tx_id BYTEA NOT NULL,
  unsigned_psbt BYTEA NOT NULL,
  signed_tx BYTEA DEFAULT NULL,
  modified_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE(account_id, id)
);

CREATE TABLE bria_batch_wallet_summaries (
  batch_id UUID REFERENCES bria_batches(id) NOT NULL,
  wallet_id UUID NOT NULL,
  total_in_sats BIGINT NOT NULL,
  total_spent_sats BIGINT NOT NULL,
  change_sats BIGINT NOT NULL,
  change_address VARCHAR NOT NULL,
  change_vout INTEGER,
  change_keychain_id UUID NOT NULL,
  fee_sats BIGINT NOT NULL,
  create_batch_ledger_tx_id UUID,
  submitted_ledger_tx_id UUID,
  modified_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE(batch_id, wallet_id)
);

CREATE TABLE bria_batch_payouts (
  batch_id UUID REFERENCES bria_batches(id) NOT NULL,
  payout_id UUID UNIQUE NOT NULL,
  UNIQUE(batch_id, payout_id)
);

CREATE TABLE bria_batch_spent_utxos (
  batch_id UUID REFERENCES bria_batches(id) NOT NULL,
  keychain_id UUID NOT NULL,
  wallet_id UUID NOT NULL,
  tx_id VARCHAR NOT NULL,
  vout INTEGER NOT NULL,
  UNIQUE(keychain_id, tx_id, vout)
);

CREATE TABLE bria_signing_sessions (
  id UUID PRIMARY KEY NOT NULL,
  account_id UUID NOT NULL,
  batch_id UUID NOT NULL,
  xpub_fingerprint BYTEA NOT NULL,
  unsigned_psbt BYTEA NOT NULL,
  FOREIGN KEY (account_id, xpub_fingerprint) REFERENCES bria_xpubs (account_id, fingerprint),
  FOREIGN KEY (account_id, batch_id) REFERENCES bria_batches (account_id, id),
  UNIQUE(account_id, batch_id, xpub_fingerprint)
);

CREATE TABLE bria_signing_session_events (
  id UUID REFERENCES bria_signing_sessions(id) NOT NULL,
  sequence INT NOT NULL,
  event_type VARCHAR NOT NULL,
  event JSONB NOT NULL,
  recorded_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE(id, sequence)
);

CREATE TABLE bria_addresses (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  address_string VARCHAR UNIQUE NOT NULL,
  profile_id UUID REFERENCES bria_profiles(id) NOT NULL,
  keychain_id UUID REFERENCES bria_wallet_keychains(id) NOT NULL,
  kind KeychainKind NOT NULL,
  address_index INTEGER NOT NULL,
  external_id VARCHAR UNIQUE,
  metadata JSONB,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  modified_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
