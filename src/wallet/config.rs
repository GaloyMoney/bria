use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletConfig {
    pub settle_income_after_n_confs: u32,
    pub settle_change_after_n_confs: u32,
}

impl WalletConfig {
    pub fn latest_income_settle_height(&self, current_height: u32) -> u32 {
        current_height - self.settle_income_after_n_confs.max(1) + 1
    }

    pub fn latest_change_settle_height(&self, current_height: u32) -> u32 {
        current_height - self.settle_change_after_n_confs.max(1) + 1
    }

    pub fn latest_settle_height(&self, current_height: u32, self_pay: bool) -> u32 {
        if self_pay {
            self.latest_change_settle_height(current_height)
        } else {
            self.latest_income_settle_height(current_height)
        }
    }
}

impl Default for WalletConfig {
    fn default() -> Self {
        Self {
            settle_income_after_n_confs: 2,
            settle_change_after_n_confs: 1,
        }
    }
}
