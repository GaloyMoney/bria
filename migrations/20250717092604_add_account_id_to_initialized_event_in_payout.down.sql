UPDATE bria_payout_events 
SET event = event - 'account_id'
WHERE event ? 'account_id';