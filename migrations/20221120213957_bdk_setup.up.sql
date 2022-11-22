CREATE TYPE BdkKeychainKind AS ENUM ('external', 'internal');

CREATE TABLE bdk_descriptor_checksums (
  keychain_id UUID NOT NULL,
  keychain_kind BdkKeychainKind NOT NULL,
  script_bytes BYTEA NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  modified_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE(keychain_id, keychain_kind)
);

CREATE TABLE bdk_script_pubkeys (
  keychain_id UUID NOT NULL,
  keychain_kind BdkKeychainKind NOT NULL,
  path INTEGER NOT NULL,
  script BYTEA NOT NULL,
  script_fmt VARCHAR NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  modified_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE(keychain_id, keychain_kind, path)
);

CREATE TABLE bdk_indexes (
  keychain_id UUID NOT NULL,
  keychain_kind BdkKeychainKind NOT NULL,
  index INTEGER NOT NULL DEFAULT 0,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  modified_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE(keychain_id, keychain_kind, index)
);
