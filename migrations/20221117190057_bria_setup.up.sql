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
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
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

CREATE TABLE bria_xpub_events (
  id UUID REFERENCES bria_xpubs(id) NOT NULL,
  sequence INT NOT NULL,
  event_type VARCHAR NOT NULL,
  event JSONB NOT NULL,
  recorded_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE(id, sequence)
);

CREATE TABLE bria_wallets (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  account_id UUID REFERENCES bria_accounts(id) NOT NULL,
  name VARCHAR NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE(account_id, name)
);

CREATE TABLE bria_wallet_events (
  id UUID REFERENCES bria_wallets(id) NOT NULL,
  sequence INT NOT NULL,
  event_type VARCHAR NOT NULL,
  event JSONB NOT NULL,
  recorded_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE(id, sequence)
);


CREATE TABLE bria_addresses (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  account_id UUID REFERENCES bria_accounts(id) NOT NULL,
  wallet_id UUID REFERENCES bria_wallets(id) NOT NULL,
  keychain_id UUID NOT NULL,
  profile_id UUID,
  address VARCHAR NOT NULL,
  address_idx INTEGER NOT NULL,
  kind KeychainKind NOT NULL,
  external_id VARCHAR NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE(account_id, address),
  UNIQUE(account_id, external_id)
);

CREATE TABLE bria_address_events (
  id UUID REFERENCES bria_addresses(id) NOT NULL,
  sequence INT NOT NULL,
  event_type VARCHAR NOT NULL,
  event JSONB NOT NULL,
  recorded_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE(id, sequence)
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

CREATE TABLE bria_batch_groups (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  account_id UUID REFERENCES bria_accounts(id) NOT NULL,
  name VARCHAR NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE(account_id, name)
);

CREATE TABLE bria_batch_group_events (
  id UUID REFERENCES bria_batch_groups(id) NOT NULL,
  sequence INT NOT NULL,
  event_type VARCHAR NOT NULL,
  event JSONB NOT NULL,
  recorded_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE(id, sequence)
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

CREATE TABLE bria_batch_spent_utxos (
  batch_id UUID REFERENCES bria_batches(id) NOT NULL,
  keychain_id UUID NOT NULL,
  wallet_id UUID NOT NULL,
  tx_id VARCHAR NOT NULL,
  vout INTEGER NOT NULL,
  UNIQUE(keychain_id, tx_id, vout)
);

CREATE TABLE bria_payouts (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  account_id UUID REFERENCES bria_accounts(id) NOT NULL,
  wallet_id UUID REFERENCES bria_wallets(id) NOT NULL,
  batch_group_id UUID REFERENCES bria_batch_groups(id) NOT NULL,
  batch_id UUID REFERENCES bria_batches(id) DEFAULT NULL,
  profile_id UUID REFERENCES bria_profiles(id) NOT NULL,
  external_id VARCHAR NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE(account_id, external_id)
);

CREATE TABLE bria_payout_events (
  id UUID REFERENCES bria_payouts(id) NOT NULL,
  sequence INT NOT NULL,
  event_type VARCHAR NOT NULL,
  event JSONB NOT NULL,
  recorded_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE(id, sequence)
);

CREATE TABLE bria_signing_sessions (
  id UUID PRIMARY KEY NOT NULL,
  account_id UUID REFERENCES bria_accounts(id) NOT NULL,
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
