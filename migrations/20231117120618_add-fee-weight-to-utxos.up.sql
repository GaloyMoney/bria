ALTER TABLE bria_utxos
ADD COLUMN origin_tx_vbytes INTEGER,
ADD COLUMN origin_tx_fee INTEGER,
ADD COLUMN trusted_origin_tx_input_tx_ids VARCHAR[] NOT NULL DEFAULT '{}';
