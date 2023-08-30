use serde::{de::DeserializeOwned, Serialize};

use super::error::EntityError;

#[derive(Debug)]
pub struct EntityEvents<T: DeserializeOwned + Serialize> {
    last_persisted_sequence: usize,
    events: Vec<T>,
}

impl<T: DeserializeOwned + Serialize + 'static> EntityEvents<T> {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            last_persisted_sequence: 0,
            events: Vec::new(),
        }
    }

    pub fn init(initial_events: impl IntoIterator<Item = T>) -> Self {
        Self {
            last_persisted_sequence: 0,
            events: initial_events.into_iter().collect(),
        }
    }

    pub fn push(&mut self, event: T) {
        self.events.push(event);
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &T> {
        self.events.iter()
    }

    pub fn into_iter(self) -> impl DoubleEndedIterator<Item = T> {
        self.events.into_iter()
    }

    pub fn load_event(
        &mut self,
        sequence: usize,
        json: serde_json::Value,
    ) -> Result<(), EntityError> {
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

    pub fn into_new_serialized_events(
        self,
        id: impl Into<uuid::Uuid>,
    ) -> impl Iterator<Item = (uuid::Uuid, i32, String, serde_json::Value)> {
        let id = id.into();
        self.events
            .into_iter()
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

    pub fn is_dirty(&self) -> bool {
        self.last_persisted_sequence != self.events.len()
    }

    pub async fn persist(
        table_name: &str,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        events: impl Iterator<Item = (uuid::Uuid, i32, String, serde_json::Value)> + '_,
    ) -> Result<(), sqlx::Error> {
        let mut query_builder = sqlx::QueryBuilder::new(format!(
            "INSERT INTO {table_name} (id, sequence, event_type, event)"
        ));
        query_builder.push_values(events, |mut builder, (id, sequence, event_type, event)| {
            builder.push_bind(id);
            builder.push_bind(sequence);
            builder.push_bind(event_type);
            builder.push_bind(event);
        });
        let query = query_builder.build();
        query.execute(&mut **tx).await?;
        Ok(())
    }
}
