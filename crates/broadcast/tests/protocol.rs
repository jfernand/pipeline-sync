use broadcast::chain::{ChainError, DeviceId, EventChain};
use broadcast::mmr_v2::MerkleRangeTreeV2;
use broadcast::protocol::{dispatch, SyncRequest, SyncResponse};
use broadcast::sha256::Sha256Hash;

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
