-- Add up migration script here
ALTER TABLE bdk_utxos ADD COLUMN deleted_at TIMESTAMPTZ;
ALTER TABLE bdk_transactions ADD COLUMN deleted_at TIMESTAMPTZ;
