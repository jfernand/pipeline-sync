#![allow(unused)]
use crate::chain::{ChainError, DeviceId, EventChain, EventEnvelope, Hash};
use crate::mmr_v2::MerkleRangeTreeV2;
use crate::sha256::Sha256Hash;
use serde::{Deserialize, Serialize};

/// Wire vocabulary mirroring `MmrSync` and `EventChain`'s conflict-resolution
/// API — the requests a peer can send to interrogate or update another peer's
/// chain.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum SyncRequest {
    GetRoot,
    GetSize,
    GetPeakHashes,
    GetNodeHash { pos: u64 },
    GetLeaf { pos: u64 },
    GetHeads,
    GetEvent { hash: Hash },
    GetResolver,
    SubmitMergeEvent {
        payload: String,
        device_id: DeviceId,
        timestamp_millis: u64,
    },
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum SyncResponse {
    Root(Option<Sha256Hash>),
    Size(u64),
    PeakHashes(Vec<(u64, u32, Sha256Hash)>),
    NodeHash(Option<Sha256Hash>),
    Leaf(Option<String>),
    Heads(Vec<Hash>),
    Event(Option<EventEnvelope>),
    Resolver(Option<DeviceId>),
    MergeResult(Result<Hash, ChainError>),
}

/// Answer a `SyncRequest` against local state. Read-only MMR queries and chain
/// queries only borrow; `SubmitMergeEvent` is the one variant that mutates
/// `chain`. Purely in-memory — no I/O — so this stays synchronous; an async
/// network loop calls it directly after decoding a request off the wire.
pub fn dispatch(tree: &MerkleRangeTreeV2, chain: &mut EventChain, request: SyncRequest) -> SyncResponse {
    match request {
        SyncRequest::GetRoot => SyncResponse::Root(tree.get_root()),
        SyncRequest::GetSize => SyncResponse::Size(tree.len()),
        SyncRequest::GetPeakHashes => SyncResponse::PeakHashes(tree.peak_hashes()),
        SyncRequest::GetNodeHash { pos } => SyncResponse::NodeHash(tree.node_hash(pos)),
        SyncRequest::GetLeaf { pos } => SyncResponse::Leaf(tree.leaf(pos)),
        SyncRequest::GetHeads => SyncResponse::Heads(chain.heads()),
        SyncRequest::GetEvent { hash } => SyncResponse::Event(chain.get_event(&hash)),
        SyncRequest::GetResolver => SyncResponse::Resolver(chain.resolver()),
        SyncRequest::SubmitMergeEvent {
            payload,
            device_id,
            timestamp_millis,
        } => SyncResponse::MergeResult(chain.add_merge_event(payload, device_id, timestamp_millis)),
    }
}
