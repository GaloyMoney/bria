use serde::{de::DeserializeOwned, Serialize};

#[derive(Clone, Debug)]
pub struct EntityEvents<T: DeserializeOwned + Serialize + Clone> {
    last_persisted_sequence: usize,
    events: Vec<T>,
}

impl<T: DeserializeOwned + Serialize + Clone> EntityEvents<T> {
    pub fn new() -> Self {
        Self {
            last_persisted_sequence: 0,
            events: Vec::new(),
        }
    }

    pub fn init(initial_event: T) -> Self {
        Self {
            last_persisted_sequence: 0,
            events: vec![initial_event],
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

    pub fn new_serialized_events(
        &self,
        id: impl Into<uuid::Uuid>,
    ) -> impl Iterator<Item = (uuid::Uuid, i32, String, serde_json::Value)> + '_ {
        let id = id.into();
        self.events
            .iter()
            .enumerate()
            .skip(self.last_persisted_sequence)
            .map(move |(i, e)| {
                let event_json = serde_json::to_value(e).expect("Could not serialize event");
                let event_type = event_json
                    .get("type")
                    .and_then(serde_json::Value::as_str)
                    .expect("Could not get type")
                    .to_owned();
                (id, (i + 1) as i32, event_type, event_json)
            })
    }
}
