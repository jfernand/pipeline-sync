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

    /// The current DAG leaves: hashes not referenced as a parent by any other event.
    /// More than one head means the chain has diverged and needs a merge event.
    fn heads(&self) -> Vec<Hash> {
        let referenced: std::collections::HashSet<&Hash> = self
            .store
            .values()
            .flat_map(|event| &event.parent_hashes)
            .collect();
        self.store
            .keys()
            .filter(|hash| !referenced.contains(hash))
            .cloned()
            .collect()
    }

    fn is_conflicted(&self) -> bool {
        self.heads().len() > 1
    }

    /// The device authorized to resolve the current conflict, if any: the lowest
    /// `DeviceId` among the diverged heads' authors. A pure function of `store`
    /// state, so any peer holding the same heads computes the same answer without
    /// coordinating — that's what prevents two devices from both authoring a
    /// resolution for the same fork.
    fn resolver(&self) -> Option<DeviceId> {
        self.heads()
            .into_iter()
            .filter_map(|hash| self.store.get(&hash).map(|event| event.device_id.clone()))
            .min()
    }

    /// Resolve the current conflict with a merge event whose parents are every
    /// diverged head, so any peer can recognize this event resolves that fork.
    /// Only the elected `resolver()` may author it.
    fn add_merge_event(
        &mut self,
        payload: String,
        device_id: DeviceId,
        timestamp_millis: u64,
    ) -> Result<Hash, ChainError> {
        let heads = self.heads();
        if heads.len() < 2 {
            return Err(ChainError::NothingToMerge);
        }
        let required = self
            .resolver()
            .expect("resolver exists whenever there are >= 2 heads");
        if device_id != required {
            return Err(ChainError::NotAuthorizedResolver {
                attempted: device_id,
                required,
            });
        }

        self.sequence += 1;
        let event = EventEnvelope::new(payload, heads, device_id, self.sequence, timestamp_millis);
        let hash = event.hash.clone();
        self.store
            .insert(event.hash.clone(), event);
        self.local_tip = Some(hash.clone());
        Ok(hash)
    }
}

#[derive(Debug, PartialEq)]
enum ChainError {
    NothingToMerge,
    NotAuthorizedResolver {
        attempted: DeviceId,
        required: DeviceId,
    },
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
