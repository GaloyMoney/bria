UPDATE bria_payout_events
SET event = jsonb_set(
  event,
  '{account_id}',
  to_jsonb(p.account_id::text)
)
FROM bria_payouts p
WHERE bria_payout_events.id = p.id
AND event_type = 'initialized';
