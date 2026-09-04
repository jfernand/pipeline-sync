#[allow(unused)]
use std::collections::HashMap;
use std::fmt::Display;
use uuid::Uuid;

struct EventChain {
    store: HashMap<Hash, EventEnvelope>,
    local_tip: Option<Hash>,
    sequence: u64,
}

#[derive(Clone, Debug)]
#[allow(unused)]
struct EventEnvelope {
    hash: Hash,
    parent_hashes: Vec<Hash>,
    device_id: DeviceId,
    sequence: u64,
    timestamp_millis: u64,
    payload: String,
}

impl EventEnvelope {
    fn new(
        payload: String,
        parent_hashes: Vec<Hash>,
        device_id: DeviceId,
        sequence: u64,
        timestamp_millis: u64,
    ) -> Self {
        Self {
            hash: Hash::new(payload.clone()),
            parent_hashes,
            device_id,
            sequence,
            timestamp_millis,
            payload,
        }
    }
}

#[derive(Eq, Hash, PartialEq, Clone, Debug)]
pub struct Hash(String);

impl Display for Hash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Hash {
    pub(crate) fn new(payload: String) -> Self {
        Hash(payload)
    }
}

#[allow(unused)]
#[derive(Clone, Debug)]
struct DeviceId(String);

impl DeviceId {
    fn random() -> DeviceId {
        DeviceId(Uuid::new_v4() .to_string())
    }
}

impl EventChain {
    fn new() -> Self {
        EventChain {
            store: HashMap::new(),
            local_tip: None,
            sequence: 0,
        }
    }

    fn add_event(&mut self, payload: String, device_id: DeviceId, timestamp_millis: u64) {
        self.sequence += 1;
        let parent_hashes = match &self.local_tip {
            None => vec![],
            Some(hash) => vec![hash.clone()],
        };
        let event = EventEnvelope::new(
            payload,
            parent_hashes,
            device_id,
            self.sequence,
            timestamp_millis,
        );
        self.store
            .insert(
                event
                    .hash
                    .clone(),
                event,
            );
    }
}

#[cfg(test)]
mod tests {
    use crate::chain::{DeviceId, EventChain};

    #[test]
    fn add_event() {
        let mut chain = EventChain::new();
        let device_id = DeviceId::random();
        chain.add_event("test".to_string(), device_id.clone(), 0);
        assert_eq!(chain.store.len(), 1);
        assert_eq!(chain.local_tip, Some(chain.store.values().next().unwrap().hash.clone()));
    }
}
