CREATE TYPE KeychainKind AS ENUM ('external', 'internal');

CREATE TABLE descriptor_checksums (
  keychain_id UUID NOT NULL,
  keychain_kind KeychainKind NOT NULL,
  script_bytes BYTEA NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  modified_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE(keychain_id, keychain_kind)
);
