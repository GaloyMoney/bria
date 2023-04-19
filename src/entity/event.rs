use serde::de::DeserializeOwned;

pub struct EntityEvents<T: DeserializeOwned> {
    last_persisted_sequence: usize,
    events: Vec<T>,
}

impl<T: DeserializeOwned> EntityEvents<T> {
    pub fn new() -> Self {
        Self {
            last_persisted_sequence: 0,
            events: Vec::new(),
        }
    }

    pub fn load_event(
        &mut self,
        sequence: usize,
        json: serde_json::Value,
    ) -> Result<(), serde_json::Error> {
        let event = serde_json::from_value(json)?;
        self.last_persisted_sequence = sequence;
        self.events.push(event);
        Ok(())
    }
}
