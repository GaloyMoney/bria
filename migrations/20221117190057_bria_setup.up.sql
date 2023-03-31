CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TYPE KeychainKind AS ENUM ('external', 'internal');

CREATE TABLE bria_admin_api_keys (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  name VARCHAR UNIQUE NOT NULL,
  encrypted_key VARCHAR NOT NULL,
  active BOOL NOT NULL DEFAULT true,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  modified_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE bria_accounts (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  journal_id UUID NOT NULL,
  name VARCHAR UNIQUE NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  modified_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE bria_account_api_keys (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  account_id UUID REFERENCES bria_accounts(id) NOT NULL,
  name VARCHAR NOT NULL,
  encrypted_key VARCHAR NOT NULL,
  active BOOL NOT NULL DEFAULT true,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  modified_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE bria_xpubs (
  account_id UUID REFERENCES bria_accounts(id) NOT NULL,
  name VARCHAR NOT NULL,
  fingerprint BYTEA NOT NULL,
  parent_fingerprint BYTEA NOT NULL,
  original VARCHAR NOT NULL,
  xpub BYTEA NOT NULL,
  derivation_path VARCHAR,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  modified_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE(account_id, xpub),
  UNIQUE(account_id, name)
);

CREATE TABLE bria_xpub_signers (
  id UUID PRIMARY KEY NOT NULL,
  version INT NOT NULL DEFAULT 1,
  account_id UUID REFERENCES bria_accounts(id) NOT NULL,
  xpub_name VARCHAR NOT NULL,
  signer_cfg JSONB NOT NULL,
  FOREIGN KEY (account_id, xpub_name) REFERENCES bria_xpubs (account_id, name),
  UNIQUE(account_id, xpub_name, version)
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
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  modified_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE(id, version),
  UNIQUE(account_id, name, version)
);

CREATE TABLE bria_wallet_utxos (
    wallet_id UUID NOT NULL,
    keychain_id UUID NOT NULL,
    tx_id VARCHAR NOT NULL,
    vout INTEGER NOT NULL,
    kind KeychainKind NOT NULL,
    address_idx INTEGER NOT NULL,
    value NUMERIC NOT NULL,
    address VARCHAR NOT NULL,
    script_hex VARCHAR NOT NULL,
    spent BOOLEAN NOT NULL,
    income_pending_ledger_tx_id UUID,
    income_settled_ledger_tx_id UUID DEFAULT NULL,
    spending_batch_id UUID DEFAULT NULL,
    block_height INTEGER DEFAULT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    modified_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(keychain_id, tx_id, vout)
);


CREATE TABLE bria_wallet_keychains (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  wallet_id UUID NOT NULL,
  account_id UUID REFERENCES bria_accounts(id) NOT NULL,
  sequence INT NOT NULL DEFAULT 0,
  keychain_cfg JSONB NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  modified_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE(wallet_id, sequence)
);

CREATE TABLE bria_batch_groups (
  id UUID NOT NULL DEFAULT gen_random_uuid(),
  version INT NOT NULL DEFAULT 1,
  account_id UUID REFERENCES bria_accounts(id) NOT NULL,
  name VARCHAR NOT NULL,
  description VARCHAR,
  batch_cfg JSONB NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  modified_at TIMESTAMPTZ NOT NULL DEFAULT now(),
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
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  modified_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE(id, version),
  UNIQUE(account_id, external_id, version)
);

CREATE TABLE bria_batches (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  batch_group_id UUID NOT NULL,
  total_fee_sats BIGINT NOT NULL,
  bitcoin_tx_id BYTEA NOT NULL,
  unsigned_psbt BYTEA NOT NULL,
  modified_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
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
  modified_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
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
