CREATE TYPE DebitOrCredit AS ENUM ('debit', 'credit');
CREATE TYPE Status AS ENUM ('active');
CREATE TYPE Layer AS ENUM ('settled', 'pending', 'encumbered');

CREATE TABLE sqlx_ledger_accounts (
  id UUID NOT NULL,
  version INT NOT NULL DEFAULT 1,
  code VARCHAR NOT NULL,
  name VARCHAR NOT NULL,
  description VARCHAR,
  status Status NOT NULL,
  normal_balance_type DebitOrCredit NOT NULL,
  metadata JSONB,
  modified_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE(id, version),
  UNIQUE(code, version),
  UNIQUE(name, version)
);

CREATE TABLE sqlx_ledger_journals (
  id UUID NOT NULL,
  version INT NOT NULL DEFAULT 1,
  name VARCHAR NOT NULL,
  description VARCHAR,
  status Status NOT NULL,
  modified_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE(id, version),
  UNIQUE(name, version)
);

CREATE TABLE sqlx_ledger_tx_templates (
  id UUID NOT NULL,
  code VARCHAR NOT NULL,
  version INT NOT NULL DEFAULT 1,
  params JSONB,
  tx_input JSONB NOT NULL,
  entries JSONB NOT NULL,
  description VARCHAR,
  metadata JSONB,
  modified_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE(id, version),
  UNIQUE(code, version)
);

CREATE TABLE sqlx_ledger_transactions (
  id UUID NOT NULL,
  version INT NOT NULL DEFAULT 1,
  journal_id UUID NOT NULL,
  tx_template_id UUID NOT NULL,
  correlation_id UUID NOT NULL,
  effective Date NOT NULL,
  external_id UUID NOT NULL,
  description VARCHAR,
  metadata JSONB,
  modified_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE(id, version)
);

CREATE TABLE sqlx_ledger_entries (
  id UUID NOT NULL,
  version INT NOT NULL DEFAULT 1,
  transaction_id UUID NOT NULL,
  account_id UUID NOT NULL,
  journal_id UUID NOT NULL,
  entry_type VARCHAR NOT NULL,
  layer Layer NOT NULL,
  units Numeric NOT NULL,
  currency VARCHAR NOT NULL,
  direction DebitOrCredit NOT NULL,
  sequence INT NOT NULL,
  description VARCHAR,
  modified_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE(id, version)
);

CREATE TABLE sqlx_ledger_balances (
  journal_id UUID NOT NULL,
  account_id UUID NOT NULL,
  entry_id UUID NOT NULL,
  currency VARCHAR NOT NULL,
  settled_dr_balance NUMERIC NOT NULL,
  settled_cr_balance NUMERIC NOT NULL,
  settled_entry_id UUID NOT NULL,
  settled_modified_at TIMESTAMPTZ NOT NULL,
  pending_dr_balance NUMERIC NOT NULL,
  pending_cr_balance NUMERIC NOT NULL,
  pending_entry_id UUID NOT NULL,
  pending_modified_at TIMESTAMPTZ NOT NULL,
  encumbered_dr_balance NUMERIC NOT NULL,
  encumbered_cr_balance NUMERIC NOT NULL,
  encumbered_entry_id UUID NOT NULL,
  encumbered_modified_at TIMESTAMPTZ NOT NULL,
  version INT NOT NULL,
  modified_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE(journal_id, account_id, currency, version)
);

CREATE TABLE sqlx_ledger_current_balances (
  journal_id UUID NOT NULL,
  account_id UUID NOT NULL,
  currency VARCHAR NOT NULL,
  version INT NOT NULL,
  UNIQUE(journal_id, account_id, currency)
);
