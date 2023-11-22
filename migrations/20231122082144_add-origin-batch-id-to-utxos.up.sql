ALTER TABLE bria_utxos
ADD COLUMN origin_tx_batch_id UUID REFERENCES bria_batches(id) DEFAULT NULL,
ADD COLUMN origin_tx_payout_queue_id UUID REFERENCES bria_payout_queues(id) DEFAULT NULL;
