//! Payjoin protocol types for Bria

pub struct PayjoinProposal {
    pub original_psbt: Vec<u8>,
    pub payjoin_psbt: Vec<u8>,
}
