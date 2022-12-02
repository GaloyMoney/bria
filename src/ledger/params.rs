use bitcoin::blockdata::transaction::{OutPoint, TxOut};
use chrono::Utc;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx_ledger::{tx_template::*, AccountId as LedgerAccountId, JournalId};
use uuid::Uuid;

use crate::primitives::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingOnchainIncomeMeta {
    pub wallet_id: WalletId,
    pub keychain_id: KeychainId,
    pub outpoint: OutPoint,
    pub txout: TxOut,
}

#[derive(Debug)]
pub struct PendingOnchainIncomeParams {
    pub journal_id: JournalId,
    pub recipient_account_id: LedgerAccountId,
    pub pending_id: Uuid,
    pub meta: PendingOnchainIncomeMeta,
}

impl PendingOnchainIncomeParams {
    pub fn defs() -> Vec<ParamDefinition> {
        vec![
            ParamDefinition::builder()
                .name("journal_id")
                .r#type(ParamDataType::UUID)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("recipient_account_id")
                .r#type(ParamDataType::UUID)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("amount")
                .r#type(ParamDataType::DECIMAL)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("external_id")
                .r#type(ParamDataType::UUID)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("meta")
                .r#type(ParamDataType::JSON)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("effective")
                .r#type(ParamDataType::DATE)
                .build()
                .unwrap(),
        ]
    }
}

impl From<PendingOnchainIncomeParams> for TxParams {
    fn from(
        PendingOnchainIncomeParams {
            journal_id,
            recipient_account_id,
            pending_id,
            meta,
        }: PendingOnchainIncomeParams,
    ) -> Self {
        let amount = Decimal::from(meta.txout.value) / SATS_PER_BTC;
        let meta = serde_json::to_value(meta).expect("Couldn't serialize meta");
        let mut params = Self::default();
        params.insert("journal_id", journal_id);
        params.insert("recipient_account_id", recipient_account_id);
        params.insert("amount", amount);
        params.insert("external_id", pending_id);
        params.insert("meta", meta);
        params.insert("effective", Utc::now().date_naive());
        params
    }
}
