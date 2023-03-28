CREATE TYPE BdkKeychainKind AS ENUM ('external', 'internal');

CREATE TABLE bdk_descriptor_checksums (
  keychain_id UUID NOT NULL,
  keychain_kind BdkKeychainKind NOT NULL,
  script_bytes BYTEA NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  modified_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE(keychain_id, keychain_kind)
);

CREATE TABLE bdk_script_pubkeys (
  keychain_id UUID NOT NULL,
  keychain_kind BdkKeychainKind NOT NULL,
  path INTEGER NOT NULL,
  script BYTEA NOT NULL,
  script_hex VARCHAR NOT NULL,
  script_fmt VARCHAR NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  modified_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE(keychain_id, keychain_kind, path),
  UNIQUE(keychain_id, script_hex)
);

CREATE TABLE bdk_indexes (
  keychain_id UUID NOT NULL,
  keychain_kind BdkKeychainKind NOT NULL,
  index INTEGER NOT NULL DEFAULT 0,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  modified_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE(keychain_id, keychain_kind, index)
);

CREATE TABLE bdk_utxos (
  keychain_id UUID NOT NULL,
  tx_id VARCHAR NOT NULL,
  vout INTEGER NOT NULL,
  is_spent BOOLEAN NOT NULL,
  utxo_json JSONB NOT NULL,
  synced_to_wallet BOOLEAN DEFAULT false,
  spent_synced_to_wallet BOOLEAN DEFAULT false,
  confirmation_synced_to_wallet BOOLEAN DEFAULT false,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  modified_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE(keychain_id, tx_id, vout)
);

CREATE TABLE bdk_transactions (
  keychain_id UUID NOT NULL,
  tx_id VARCHAR NOT NULL,
  details_json JSONB NOT NULL,
  UNIQUE(keychain_id, tx_id)
);

CREATE TABLE bdk_sync_times (
  keychain_id UUID UNIQUE NOT NULL,
  height INTEGER NOT NULL,
  timestamp INTEGER NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  modified_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
