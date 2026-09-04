#![allow(unused)]
use crate::sha256::Sha256Hash;
use std::collections::HashMap;

/// A node in the Merkle Mountain Range
#[derive(Debug, Clone)]
struct MmrNode {
    hash: Sha256Hash,
    position: u64,
}

#[derive(Clone, Debug)]
/// Simple Merkle Mountain Range implementation
/// An MMR is an append-only data structure that maintains a forest of perfect binary trees
pub struct MerkleRangeTree {
    /// Storage for all nodes in the MMR
    nodes: HashMap<u64, Sha256Hash>,
    /// Current size (number of leaves)
    size: u64,
}

impl MerkleRangeTree {
    /// Create a new empty MMR
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            size: 0,
        }
    }

    /// Append a new leaf to the MMR
    pub fn append(&mut self, hash: Sha256Hash) -> u64 {
        let pos = self.size;
        self.nodes.insert(pos, hash);
        self.size += 1;

        // Merge peaks if needed
        let mut height = 0;
        let mut current_pos = pos;

        while self.can_merge(current_pos, height) {
            let left_sibling = current_pos + 1 - (1 << (height + 1));
            let parent_hash = self.merge_hashes(
                self.nodes.get(&left_sibling).unwrap(),
                self.nodes.get(&current_pos).unwrap(),
            );

            current_pos += 1;
            self.nodes.insert(current_pos, parent_hash);
            height += 1;
        }

        pos
    }

    /// Check if we can merge at the given position and height
    fn can_merge(&self, pos: u64, height: u32) -> bool {
        let left_sibling = if pos < (1 << (height + 1)) - 1 {
            return false;
        } else {
            pos - ((1 << (height + 1)) - 1)
        };

        self.nodes.contains_key(&left_sibling)
    }

    /// Merge two hashes to create parent hash
    fn merge_hashes(&self, left: &Sha256Hash, right: &Sha256Hash) -> Sha256Hash {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(left.as_bytes());
        hasher.update(right.as_bytes());
        Sha256Hash::new(hasher.finalize().into())
    }

    /// Get the peak positions for the current MMR
    pub fn get_peaks(&self) -> Vec<u64> {
        let mut peaks = Vec::new();
        let mut size = self.size;
        let mut pos = 0;

        while size > 0 {
            let height = size.trailing_zeros();
            let peak_size = (1u64 << (height + 1)) - 1;
            pos += peak_size;
            peaks.push(pos - 1);
            size -= 1 << height;
        }

        peaks
    }

    /// Calculate the root hash by bagging all peaks
    pub fn get_root(&self) -> Option<Sha256Hash> {
        let peaks = self.get_peaks();

        if peaks.is_empty() {
            return None;
        }

        let mut peak_hashes: Vec<Sha256Hash> = peaks
            .iter()
            .filter_map(|&pos| self.nodes.get(&pos).cloned())
            .collect();

        // Bag the peaks from right to left
        while peak_hashes.len() > 1 {
            let right = peak_hashes.pop().unwrap();
            let left = peak_hashes.pop().unwrap();
            peak_hashes.push(self.merge_hashes(&left, &right));
        }

        peak_hashes.into_iter().next()
    }

    /// Get the number of leaves in the MMR
    pub fn len(&self) -> u64 {
        self.size
    }

    /// Check if the MMR is empty
    pub fn is_empty(&self) -> bool {
        self.size == 0
    }
}

#[cfg(test)]
mod tests {
    use crate::mmr::MerkleRangeTree;

    #[test]
    pub fn test() {
        let mut mmr = MerkleRangeTree::new();
        assert_eq!(mmr.len(), 0);
        assert!(mmr.is_empty());
        mmr.append("a".into());
        dbg!(&mmr);
        mmr.append("b".into());
        dbg!(&mmr);
        mmr.append("c".into());
        dbg!(&mmr);
        dbg!(&mmr.get_root());
        dbg!(&mmr.get_peaks());
    }
}
