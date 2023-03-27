 SELECT u.tx_id, vout FROM bdk_utxos u JOIN bdk_transactions t ON u.keychain_id = t.keychain_id AND u.tx_id = t.tx_id
 WHERE u.keychain_id = "2c95c3e0-96fa-4ac4-a003-bb2cd3c00a17"
 AND ledger_tx_settled_id IS NULL
 AND ledger_tx_pending_id IS NOT NULL
 AND (details_json->'confirmation_time'->'height')::INTEGER > 0
 AND NOT (u.tx_id || ':' || vout::TEXT = ANY([]))
 LIMIT 1
