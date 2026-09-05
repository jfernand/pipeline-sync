#[allow(unused)]
use crate::sha256::Sha256Hash;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Display;
use uuid::Uuid;

pub(crate) struct EventChain {
    store: HashMap<Hash, EventEnvelope>,
    local_tip: Option<Hash>,
    sequence: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[allow(unused)]
pub(crate) struct EventEnvelope {
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

#[derive(Eq, Hash, PartialEq, PartialOrd, Ord, Clone, Debug, Serialize, Deserialize)]
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
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) struct DeviceId(String);

impl DeviceId {
    pub(crate) fn random() -> DeviceId {
        DeviceId(Uuid::new_v4() .to_string())
    }

    pub(crate) fn new(id: String) -> DeviceId {
        DeviceId(id)
    }
}

impl EventChain {
    pub(crate) fn new() -> Self {
        EventChain {
            store: HashMap::new(),
            local_tip: None,
            sequence: 0,
        }
    }

    pub(crate) fn add_event(&mut self, payload: String, device_id: DeviceId, timestamp_millis: u64) {
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

    /// Look up a stored event by its hash, for transferring it to a peer.
    pub(crate) fn get_event(&self, hash: &Hash) -> Option<EventEnvelope> {
        self.store.get(hash).cloned()
    }

    /// The current DAG leaves: hashes not referenced as a parent by any other event.
    /// More than one head means the chain has diverged and needs a merge event.
    pub(crate) fn heads(&self) -> Vec<Hash> {
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

    pub(crate) fn is_conflicted(&self) -> bool {
        self.heads().len() > 1
    }

    /// The device authorized to resolve the current conflict, if any: the lowest
    /// `DeviceId` among the diverged heads' authors. A pure function of `store`
    /// state, so any peer holding the same heads computes the same answer without
    /// coordinating — that's what prevents two devices from both authoring a
    /// resolution for the same fork.
    pub(crate) fn resolver(&self) -> Option<DeviceId> {
        self.heads()
            .into_iter()
            .filter_map(|hash| self.store.get(&hash).map(|event| event.device_id.clone()))
            .min()
    }

    /// Resolve the current conflict with a merge event whose parents are every
    /// diverged head, so any peer can recognize this event resolves that fork.
    /// Only the elected `resolver()` may author it.
    pub(crate) fn add_merge_event(
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

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub(crate) enum ChainError {
    NothingToMerge,
    NotAuthorizedResolver {
        attempted: DeviceId,
        required: DeviceId,
    },
}

#[cfg(test)]
mod tests {
    use crate::chain::{ChainError, DeviceId, EventChain, EventEnvelope};

    #[test]
    fn add_event() {
        let mut chain = EventChain::new();
        let device_id = DeviceId::random();
        chain.add_event("test".to_string(), device_id.clone(), 0);
        assert_eq!(chain.store.len(), 1);
        assert_eq!(chain.local_tip, Some(chain.store.values().next().unwrap().hash.clone()));
    }

    /// Build a chain with a single root event, then two events from two different
    /// devices that both list the root as their sole parent — a fork.
    fn forked_chain() -> (EventChain, DeviceId, DeviceId) {
        let mut chain = EventChain::new();
        let root_device = DeviceId::random();
        chain.add_event("root".to_string(), root_device, 0);
        let root = chain.local_tip.clone().unwrap();

        let (low, high) = {
            let a = DeviceId::random();
            let b = DeviceId::random();
            if a < b { (a, b) } else { (b, a) }
        };

        let event_low = EventEnvelope::new("from low".to_string(), vec![root.clone()], low.clone(), 1, 1);
        let event_high = EventEnvelope::new("from high".to_string(), vec![root], high.clone(), 1, 1);
        chain.store.insert(event_low.hash.clone(), event_low);
        chain.store.insert(event_high.hash.clone(), event_high);

        (chain, low, high)
    }

    #[test]
    fn heads_diverge_on_fork() {
        let (chain, _low, _high) = forked_chain();
        assert_eq!(chain.heads().len(), 2);
        assert!(chain.is_conflicted());
    }

    #[test]
    fn resolver_is_deterministic() {
        let (chain, low, _high) = forked_chain();
        assert_eq!(chain.resolver(), Some(low));
    }

    #[test]
    fn add_merge_event_by_resolver_succeeds() {
        let (mut chain, low, _high) = forked_chain();
        let result = chain.add_merge_event("resolved".to_string(), low, 2);
        assert!(result.is_ok());
        let merge_hash = result.unwrap();
        assert_eq!(chain.heads(), vec![merge_hash.clone()]);
        assert_eq!(chain.local_tip, Some(merge_hash));
    }

    #[test]
    fn add_merge_event_by_non_resolver_fails() {
        let (mut chain, low, high) = forked_chain();
        let heads_before = {
            let mut heads = chain.heads();
            heads.sort();
            heads
        };

        let result = chain.add_merge_event("resolved".to_string(), high.clone(), 2);
        assert_eq!(
            result,
            Err(ChainError::NotAuthorizedResolver {
                attempted: high,
                required: low,
            })
        );

        let mut heads_after = chain.heads();
        heads_after.sort();
        assert_eq!(heads_after, heads_before);
    }
}
