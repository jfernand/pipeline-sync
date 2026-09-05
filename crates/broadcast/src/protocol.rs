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

#[cfg(test)]
mod tests {
    use super::{dispatch, SyncRequest, SyncResponse};
    use crate::chain::{ChainError, DeviceId, EventChain};
    use crate::mmr_v2::MerkleRangeTreeV2;
    use crate::sha256::Sha256Hash;

    fn roundtrip_request(request: SyncRequest) {
        let bytes = postcard::to_allocvec(&request).unwrap();
        let decoded: SyncRequest = postcard::from_bytes(&bytes).unwrap();
        assert_eq!(decoded, request);
    }

    fn roundtrip_response(response: SyncResponse) {
        let bytes = postcard::to_allocvec(&response).unwrap();
        let decoded: SyncResponse = postcard::from_bytes(&bytes).unwrap();
        assert_eq!(decoded, response);
    }

    #[test]
    fn requests_roundtrip() {
        roundtrip_request(SyncRequest::GetRoot);
        roundtrip_request(SyncRequest::GetSize);
        roundtrip_request(SyncRequest::GetPeakHashes);
        roundtrip_request(SyncRequest::GetNodeHash { pos: 3 });
        roundtrip_request(SyncRequest::GetLeaf { pos: 3 });
        roundtrip_request(SyncRequest::GetHeads);
        roundtrip_request(SyncRequest::GetResolver);
        roundtrip_request(SyncRequest::SubmitMergeEvent {
            payload: "resolved".into(),
            device_id: DeviceId::new("device-a".into()),
            timestamp_millis: 42,
        });
    }

    #[test]
    fn responses_roundtrip() {
        let hash: Sha256Hash = "a".into();
        roundtrip_response(SyncResponse::Root(Some(hash)));
        roundtrip_response(SyncResponse::Root(None));
        roundtrip_response(SyncResponse::Size(4));
        roundtrip_response(SyncResponse::PeakHashes(vec![(2, 1, hash), (3, 0, hash)]));
        roundtrip_response(SyncResponse::NodeHash(Some(hash)));
        roundtrip_response(SyncResponse::Leaf(Some("a".into())));
        roundtrip_response(SyncResponse::Heads(vec![]));
        roundtrip_response(SyncResponse::Resolver(Some(DeviceId::new("device-a".into()))));
        roundtrip_response(SyncResponse::MergeResult(Err(ChainError::NothingToMerge)));
    }

    fn mmr_fixture() -> MerkleRangeTreeV2 {
        let mut tree = MerkleRangeTreeV2::new();
        tree.append("a".into());
        tree.append("b".into());
        tree
    }

    #[test]
    fn dispatch_answers_mmr_queries() {
        let tree = mmr_fixture();
        let mut chain = EventChain::new();

        assert_eq!(
            dispatch(&tree, &mut chain, SyncRequest::GetRoot),
            SyncResponse::Root(tree.get_root())
        );
        assert_eq!(
            dispatch(&tree, &mut chain, SyncRequest::GetSize),
            SyncResponse::Size(2)
        );
        assert_eq!(
            dispatch(&tree, &mut chain, SyncRequest::GetLeaf { pos: 0 }),
            SyncResponse::Leaf(Some("a".to_string()))
        );
    }

    #[test]
    fn dispatch_answers_chain_queries_and_submits_merge() {
        let tree = mmr_fixture();
        let mut chain = EventChain::new();
        let root_device = DeviceId::random();
        chain.add_event("root".into(), root_device, 0);

        assert_eq!(
            dispatch(&tree, &mut chain, SyncRequest::GetHeads),
            SyncResponse::Heads(chain.heads())
        );
        assert_eq!(
            dispatch(&tree, &mut chain, SyncRequest::GetResolver),
            SyncResponse::Resolver(chain.resolver())
        );

        // Only one head exists yet, so a merge attempt is rejected.
        let device = DeviceId::random();
        assert_eq!(
            dispatch(
                &tree,
                &mut chain,
                SyncRequest::SubmitMergeEvent {
                    payload: "resolved".into(),
                    device_id: device,
                    timestamp_millis: 1,
                }
            ),
            SyncResponse::MergeResult(Err(ChainError::NothingToMerge))
        );
    }
}
