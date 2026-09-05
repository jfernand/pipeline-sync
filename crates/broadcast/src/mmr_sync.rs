#![allow(unused)]
use crate::mmr_v2::MerkleRangeTreeV2;
use crate::sha256::Sha256Hash;

/// Read-side interrogation surface for syncing an append-only MMR-backed log.
///
/// Implemented directly by local trees today; a network-backed implementor
/// (sending these same calls as protocol messages, see `protocol::SyncRequest`)
/// can satisfy it later without changing callers.
pub trait MmrSync {
    async fn root(&self) -> Option<Sha256Hash>;
    async fn size(&self) -> u64;
    /// Positions + height + hash — what's actually compared/verified against a peer.
    async fn peak_hashes(&self) -> Vec<(u64, u32, Sha256Hash)>;
    async fn node_hash(&self, pos: u64) -> Option<Sha256Hash>;
    async fn leaf(&self, pos: u64) -> Option<String>;
}

impl MmrSync for MerkleRangeTreeV2 {
    async fn root(&self) -> Option<Sha256Hash> {
        self.get_root()
    }

    async fn size(&self) -> u64 {
        self.len()
    }

    async fn peak_hashes(&self) -> Vec<(u64, u32, Sha256Hash)> {
        self.peak_hashes()
    }

    async fn node_hash(&self, pos: u64) -> Option<Sha256Hash> {
        self.node_hash(pos)
    }

    async fn leaf(&self, pos: u64) -> Option<String> {
        self.leaf(pos)
    }
}

#[cfg(test)]
mod tests {
    use super::MmrSync;
    use crate::mmr_v2::MerkleRangeTreeV2;

    #[tokio::test]
    async fn trait_methods_agree_with_inherent_methods() {
        let mut mmr = MerkleRangeTreeV2::new();
        mmr.append("a".into());
        mmr.append("b".into());
        mmr.append("c".into());
        mmr.append("d".into());

        assert_eq!(MmrSync::root(&mmr).await, mmr.get_root());
        assert_eq!(MmrSync::size(&mmr).await, mmr.len());
        assert_eq!(MmrSync::peak_hashes(&mmr).await, mmr.peak_hashes());
        for pos in 0..mmr.nodes() as u64 {
            assert_eq!(MmrSync::node_hash(&mmr, pos).await, mmr.node_hash(pos));
            assert_eq!(MmrSync::leaf(&mmr, pos).await, mmr.leaf(pos));
        }
    }
}
