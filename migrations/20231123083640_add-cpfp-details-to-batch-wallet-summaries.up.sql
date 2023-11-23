ALTER TABLE bria_batch_wallet_summaries
RENAME COLUMN fee_sats TO total_fee_sats;

ALTER TABLE bria_batch_wallet_summaries
ADD COLUMN cpfp_fee_sats BIGINT NOT NULL;

ALTER TABLE bria_batch_wallet_summaries
ADD COLUMN cpfp_details JSONB NOT NULL;
