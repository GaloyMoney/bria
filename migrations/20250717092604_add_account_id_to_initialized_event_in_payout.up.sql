UPDATE bria_payout_events
SET event = jsonb_set(
  event,
  '{account_id}',
  to_jsonb(
    (SELECT account_id FROM bria_payouts WHERE id = bria_payout_events.id)::text
  )
)
WHERE event_type = 'initialized';
