#![allow(unused)]
use crate::sha256::Sha256Hash;
use serde::{Deserialize, Serialize};

/// Wire vocabulary mirroring `MmrSync` — the requests a peer can send to
/// interrogate another peer's chain.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum SyncRequest {
    GetRoot,
    GetSize,
    GetPeakHashes,
    GetNodeHash { pos: u64 },
    GetLeaf { pos: u64 },
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum SyncResponse {
    Root(Option<Sha256Hash>),
    Size(u64),
    PeakHashes(Vec<(u64, u32, Sha256Hash)>),
    NodeHash(Option<Sha256Hash>),
    Leaf(Option<String>),
}

#[cfg(test)]
mod tests {
    use super::{SyncRequest, SyncResponse};
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
    }
}
