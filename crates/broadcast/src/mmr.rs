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
    /// Storage for all nodes in the MMR, keyed by their postorder position
    /// (leaves *and* merge nodes each occupy one position).
    nodes: BTreeMap<u64, (Sha256Hash, u32, String)>,
    /// Current size (number of leaves)
    size: u64,
    /// Next free postorder position, i.e. total nodes (leaves + merges) ever inserted
    next_pos: u64,
}

impl MerkleRangeTree {
    /// Create a new empty MMR
    pub fn new() -> Self {
        Self {
            nodes: BTreeMap::new(),
            size: 0,
            next_pos: 0,
        }
    }

    /// Append a new leaf to the MMR
    pub fn append(&mut self, value: String) -> (u64, u32) {
        let hash: Sha256Hash = value
            .as_str()
            .into();
        let pos = self.next_pos;
        self.next_pos += 1;
        self.nodes
            .insert(pos, (hash, 0, value));
        self.size += 1;

        // Merge peaks if needed
        let mut height = 0;
        let mut current_pos = pos;

        while let Some(left_sibling) = self.can_merge(current_pos, height) {
            let (left, _, _) = self
                .nodes
                .get(&left_sibling)
                .unwrap();
            let (right, _, _) = self
                .nodes
                .get(&current_pos)
                .unwrap();
            let parent_hash = *left + *right;
            let parent_value = format!("{left}+{right}");

            height += 1;
            current_pos = self.next_pos;
            self.next_pos += 1;
            self.nodes
                .insert(current_pos, (parent_hash, height, parent_value));
        }

        (pos, height)
    }

    // Determine the position of the left sibling of a node at a given height and position
    fn left_sibling(height: u32, current_pos: u64) -> Option<u64> {
        current_pos.checked_sub((1u64 << (height + 1)) - 1)
    }

    /// Check whether the node at `pos`/`height` has a same-height sibling to merge with,
    /// returning that sibling's position if so.
    ///
    /// A position existing at the computed offset isn't enough on its own: postorder
    /// positions are shared by leaves and internal nodes, so the offset can land on an
    /// unrelated node from a previously-closed subtree. Its height must match too.
    fn can_merge(&self, pos: u64, height: u32) -> Option<u64> {
        let left_sibling = Self::left_sibling(height, pos)?;

        match self.nodes.get(&left_sibling) {
            Some((_, sibling_height, _)) if *sibling_height == height => Some(left_sibling),
            _ => None,
        }
    }

    /// Get the peak positions for the current MMR
    pub fn get_peaks(&self) -> Vec<u64> {
        let mut peaks = Vec::new();
        let mut size = self.size;
        let mut pos = 0;

        // Peaks occupy positions left to right in *decreasing* size order (the
        // largest peak is built, and thus closes, first), so bits must be taken
        // off from the highest set bit down — not the lowest, which would put
        // a smaller peak's position range before a larger one that precedes it.
        while size > 0 {
            let height = 63 - size.leading_zeros();
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
                    .map(|(hash, _, _)| hash)
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
        let nodes = self.nodes.iter().map(|(k, (h, height, v))| format!("{}, h{}, {}, {}\n", k, height, h, v)).collect::<Vec<String>>().join("");
        write!(f, "{}\n{}\n }}", header, nodes)
    }
}
#[cfg(test)]
mod tests {
    use crate::mmr::MerkleRangeTree;

    #[test]
    pub fn test_left_sibling() {
        // node 5 (c+d, height 1) sibling-checks against node 2 (a+b, height 1)
        // in the 4-leaf tree built by `test` below.
        assert_eq!(MerkleRangeTree::left_sibling(1, 5), Some(2));
        // out-of-range combinations must not underflow the position arithmetic.
        assert_eq!(MerkleRangeTree::left_sibling(2, 3), None);
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
        // c has no sibling yet (node 2 is height 1, not height 0), so it stays its own peak.
        assert_eq!(height, 0, "h(3)");
        assert_eq!(pos, 3, "pos(3)");
        assert_eq!(mmr.len(), 3); // 0b11
        assert_eq!(mmr.nodes(), 4);
        // peaks are (a+b) at 2 and c at 3 — not a lone `a` at 0.
        assert_eq!(&mmr.get_peaks(), &[2, 3]);

        let (pos, height) =mmr.append("d".into());
        dbg!(&mmr);
        dbg!(&mmr.get_root());
        dbg!(&mmr.get_peaks());
        // d merges with c (height 0), then (c+d) merges with (a+b) (height 1).
        assert_eq!(height, 2, "h(4)");
        assert_eq!(pos, 4, "pos(4)");
        assert_eq!(mmr.len(), 4); //0b100
        assert_eq!(mmr.nodes(), 7);
        assert_eq!(&mmr.get_peaks(), &[6])
    }
}
