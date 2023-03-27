use sqlx::{Pool, Postgres};

use super::entity::*;
use crate::error::*;

pub struct WalletUtxos {
    _pool: Pool<Postgres>,
}

impl WalletUtxos {
    pub async fn persist(&self, _utxo: NewWalletUtxo) -> Result<WalletUtxo, BriaError> {
        unimplemented!()
    }
}
