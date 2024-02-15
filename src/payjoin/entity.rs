struct ForeignPayjoinInput {
    pub txid: bitcoin::Txid,
    pub vout: u32,
    pub value: u64,
    pub script_pubkey: bitcoin::Script,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PayjoinEvent {
    Initialized {
        foreign_utxos: Vec<ForeignPayjoinInput>,
    },
}

pub struct PayJoin {
    pub id: PayjoinId,

    pub(super) events: EntityEvents<PayjoinEvent>,
}
