ALTER TABLE bria_xpub_events ADD COLUMN context JSONB DEFAULT NULL;
ALTER TABLE bria_wallet_events ADD COLUMN context JSONB DEFAULT NULL;
ALTER TABLE bria_address_events ADD COLUMN context JSONB DEFAULT NULL;
ALTER TABLE bria_payout_queue_events ADD COLUMN context JSONB DEFAULT NULL;
ALTER TABLE bria_payout_events ADD COLUMN context JSONB DEFAULT NULL;
ALTER TABLE bria_signing_session_events ADD COLUMN context JSONB DEFAULT NULL;
ALTER TABLE bria_profile_events ADD COLUMN context JSONB DEFAULT NULL;
