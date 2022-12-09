CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TABLE admin_api_keys (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  name VARCHAR UNIQUE NOT NULL,
  encrypted_key VARCHAR NOT NULL,
  active BOOL NOT NULL DEFAULT true,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  modified_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE accounts (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  journal_id UUID NOT NULL,
  name VARCHAR UNIQUE NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  modified_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE account_api_keys (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  account_id UUID REFERENCES accounts(id) NOT NULL,
  name VARCHAR UNIQUE NOT NULL,
  encrypted_key VARCHAR NOT NULL,
  active BOOL NOT NULL DEFAULT true,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  modified_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE xpubs (
  account_id UUID REFERENCES accounts(id) NOT NULL,
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

CREATE TABLE keychains (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  account_id UUID REFERENCES accounts(id) NOT NULL,
  keychain_cfg JSONB NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  modified_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE wallets (
  id UUID NOT NULL DEFAULT gen_random_uuid(),
  version INT NOT NULL DEFAULT 1,
  account_id UUID REFERENCES accounts(id) NOT NULL,
  ledger_account_id UUID NOT NULL,
  dust_ledger_account_id UUID NOT NULL,
  keychain_id UUID REFERENCES keychains(id) NOT NULL,
  name VARCHAR NOT NULL,
  wallet_cfg JSONB NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  modified_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE(id, version),
  UNIQUE(account_id, name, version)
);

CREATE TABLE batch_groups (
  id UUID NOT NULL DEFAULT gen_random_uuid(),
  version INT NOT NULL DEFAULT 1,
  account_id UUID REFERENCES accounts(id) NOT NULL,
  name VARCHAR NOT NULL,
  description VARCHAR,
  batch_cfg JSONB NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  modified_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE(id, version),
  UNIQUE(account_id, name, version)
);

CREATE TABLE batches (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid()
);

CREATE TABLE payouts (
  id UUID NOT NULL DEFAULT gen_random_uuid(),
  version INT NOT NULL DEFAULT 1,
  account_id UUID REFERENCES accounts(id) NOT NULL,
  wallet_id UUID NOT NULL,
  batch_group_id UUID NOT NULL,
  destination_data JSONB NOT NULL,
  satoshis BIGINT NOT NULL,
  batch_id UUID REFERENCES batches(id) DEFAULT NULL,
  priority INT NOT NULL DEFAULT 100,
  external_id VARCHAR NOT NULL,
  metadata JSONB,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  modified_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE(id, version),
  UNIQUE(account_id, external_id, version)
);

CREATE INDEX ON payouts (batch_group_id) WHERE batch_id IS NULL;
