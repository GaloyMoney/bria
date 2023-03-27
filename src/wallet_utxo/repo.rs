use sqlx::{Pool, Postgres};

use super::entity::*;
use crate::error::*;

pub(super) struct WalletUtxoRepo {
    _pool: Pool<Postgres>,
}

impl WalletUtxoRepo {
    pub async fn persist(&self, _utxo: NewWalletUtxo) -> Result<WalletUtxo, BriaError> {
        unimplemented!()
    }
}
