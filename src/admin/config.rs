use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AdminApiConfig {
    #[serde(default = "default_port")]
    pub listen_port: u16,
}
impl Default for AdminApiConfig {
    fn default() -> Self {
        Self {
            listen_port: default_port(),
        }
    }
}

fn default_port() -> u16 {
    2743
}
