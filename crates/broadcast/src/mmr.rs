#![allow(unused)]
use crate::sha256::Sha256Hash;
use std::collections::{BTreeMap, HashMap};
use std::fmt::Debug;

/// A node in the Merkle Mountain Range
#[derive(Debug, Clone)]
struct MmrNode {
    hash: Sha256Hash,
    value: Option<String>,
    position: u64,
}

#[derive(Clone)]
/// Simple Merkle Mountain Range implementation
/// An MMR is an append-only data structure that maintains a forest of perfect binary trees
pub struct MerkleRangeTree {
    /// Storage for all nodes in the MMR
    nodes: BTreeMap<u64, (Sha256Hash, String)>,
    /// Current size (number of leaves)
    size: u64,
}

impl MerkleRangeTree {
    /// Create a new empty MMR
    pub fn new() -> Self {
        Self {
            nodes: BTreeMap::new(),
            size: 0,
        }
    }

    /// Append a new leaf to the MMR
    pub fn append(&mut self, value: String) -> (u64, u32) {
        let hash: Sha256Hash = value
            .as_str()
            .into();
        let pos = self.size;
        self.nodes
            .insert(pos, (hash, value));
        self.size += 1;

        // Merge peaks if needed
        let mut height = 0;
        let mut current_pos = pos;

        while self.can_merge(current_pos, height) {
            let left_sibling = Self::left_sibling(height, current_pos);
            let (left, _) = self
                .nodes
                .get(&left_sibling)
                .unwrap();
            let (right, _) = self
                .nodes
                .get(&current_pos)
                .unwrap();
            let parent_hash = *left + *right;

            current_pos += 1;
            self.nodes
                .insert(current_pos, (parent_hash, format!("{left}+{right}").to_string()));
            height += 1;
        }

        (pos, height)
    }

    // Determine the position of the left sibling of a node at a given height and position
    fn left_sibling(height: u32, current_pos: u64) -> u64 {
        current_pos + 1 - (1 << (height + 1))
    }

    /// Check if we can merge at the given position and height
    fn can_merge(&self, pos: u64, height: u32) -> bool {
        let left_sibling = if pos < (1 << (height + 1)) - 1 {
            return false;
        } else {
            pos - ((1 << (height + 1)) - 1)
        };

        self.nodes
            .contains_key(&left_sibling)
    }

    /// Merge two hashes to create parent hash
    fn merge_hashes(&self, left: &Sha256Hash, right: &Sha256Hash) -> Sha256Hash {
        // use sha2::{Digest, Sha256};
        // let mut hasher = Sha256::new();
        // hasher.update(left.as_bytes());
        // hasher.update(right.as_bytes());
        // Sha256Hash::new(hasher.finalize().into())
        *left + *right
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
            .filter_map(|&pos| {
                self.nodes
                    .get(&pos)
                    .cloned()
                    .map(|(hash, _)| hash)
            })
            .collect();

        // Bag the peaks from right to left
        while peak_hashes.len() > 1 {
            let right = peak_hashes
                .pop()
                .unwrap();
            let left = peak_hashes
                .pop()
                .unwrap();
            peak_hashes.push(left + right);
        }

        peak_hashes
            .into_iter()
            .next()
    }

    /// Get the number of leaves in the MMR
    pub fn len(&self) -> u64 {
        self.size
    }

    pub fn nodes(&self) -> usize {
        self.nodes
            .len()
    }

    /// Check if the MMR is empty
    pub fn is_empty(&self) -> bool {
        self.size == 0
    }
}

impl Debug for MerkleRangeTree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let header = format!("MerkleRangeTree {{ size: {}", self.size);
        let nodes = self.nodes.iter().map(|(k, (h,v))| format!("{}, {}, {}\n", k, h, v)).collect::<Vec<String>>().join("");
        write!(f, "{}\n{}\n }}", header, nodes)
    }
}
#[cfg(test)]
mod tests {
    use crate::mmr::MerkleRangeTree;

    #[test]
    pub fn test_left_sibling() {
        let height = 2;
        let current_pos = 3;
        let left_sibling = MerkleRangeTree::left_sibling(height, current_pos);
        assert_eq!(left_sibling, 1);
    }

    #[test]
    pub fn test() {
        let mut mmr = MerkleRangeTree::new();
        assert_eq!(mmr.len(), 0);
        assert!(mmr.is_empty());
        let (pos, height) = mmr.append("a".into());
        dbg!(&mmr);
        dbg!(&mmr.get_root());
        dbg!(&mmr.get_peaks());
        assert_eq!(pos, 0);
        assert_eq!(height, 0);
        assert_eq!(mmr.len(), 1);
        assert_eq!(&mmr.get_peaks(), &[0]);

        let (pos, height) = mmr.append("b".into());
        dbg!(&mmr);
        dbg!(&mmr.get_root());
        dbg!(&mmr.get_peaks());
        assert_eq!(pos, 1);
        assert_eq!(height, 1);
        assert_eq!(mmr.len(), 2); // 0b10
        assert_eq!(mmr.nodes(), 3);
        assert_eq!(&mmr.get_peaks(), &[2]);

        let (pos, height) =mmr.append("c".into());
        dbg!("c", &mmr);
        dbg!(&mmr.get_root());
        dbg!(&mmr.get_peaks());
        assert_eq!(height, 2, "h(3)");
        assert_eq!(pos, 2, "pos(3)");
        assert_eq!(mmr.len(), 3); // 0b11
        assert_eq!(mmr.nodes(), 5);
        assert_eq!(&mmr.get_peaks(), &[0, 3]);

        let (pos, height) =mmr.append("d".into());
        dbg!(&mmr);
        dbg!(&mmr.get_root());
        dbg!(&mmr.get_peaks());
        assert_eq!(mmr.len(), 4); //0b100
        assert_eq!(mmr.nodes(), 6);
        assert_eq!(&mmr.get_peaks(), &[6])
    }
}
