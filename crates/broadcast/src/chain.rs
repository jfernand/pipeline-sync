#[allow(unused)]
use crate::sha256::Sha256Hash;
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
        mut parent_hashes: Vec<Hash>,
        device_id: DeviceId,
        sequence: u64,
        timestamp_millis: u64,
    ) -> Self {
        parent_hashes.sort();
        let hash = Self::compute_hash(
            &parent_hashes,
            &device_id,
            sequence,
            timestamp_millis,
            &payload,
        );
        Self {
            hash,
            parent_hashes,
            device_id,
            sequence,
            timestamp_millis,
            payload,
        }
    }

    fn compute_hash(
        parent_hashes: &[Hash],
        device_id: &DeviceId,
        sequence: u64,
        timestamp_millis: u64,
        payload: &str,
    ) -> Hash {
        let canonical = format!(
            "{:?}|{:?}|{}|{}|{}",
            parent_hashes, device_id, sequence, timestamp_millis, payload
        );
        Hash::new(Sha256Hash::from(canonical.as_str()).to_hex().into())
    }
}

#[derive(Eq, Hash, PartialEq, PartialOrd, Ord, Clone, Debug)]
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
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
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
        let event_hash = event.hash.clone();
        self.store
            .insert(
                event
                    .hash
                    .clone(),
                event,
            );
        self.local_tip = Some(event_hash);
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
