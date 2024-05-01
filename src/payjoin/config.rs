use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PayjoinConfig {
    #[serde(default = "default_port")]
    pub listen_port: u16,
}
impl Default for PayjoinConfig {
    fn default() -> Self {
        Self {
            listen_port: default_port(),
        }
    }
}

fn default_port() -> u16 {
    8088
}
