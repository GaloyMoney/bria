UPDATE sqlx_ledger_transactions
SET metadata = jsonb_set(
  metadata,
  '{tx_summary}',
  CASE
  WHEN metadata->'batch_info' ? 'cpfp_details' AND metadata->'batch_info' ? 'cpfp_fee_sats' THEN
    jsonb_set(
      metadata->'tx_summary',
      '{cpfp_details}', 
      metadata->'batch_info'->'cpfp_details',
      true
      ) || jsonb_build_object(
      'cpfp_fee_sats', metadata->'batch_info'->'cpfp_fee_sats'
    )
  ELSE
    metadata->'tx_summary'
  END,
  true
)
WHERE metadata->'batch_info' ? 'cpfp_details' AND metadata->'batch_info' ? 'cpfp_fee_sats';
