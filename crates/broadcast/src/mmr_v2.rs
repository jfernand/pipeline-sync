#![allow(unused)]
use crate::sha256::Sha256Hash;
use std::collections::BTreeMap;
use std::fmt::Debug;

/// Merkle Mountain Range built around an explicit peak stack, instead of
/// recovering sibling relationships from position arithmetic (see `mmr.rs`).
///
/// Each append pushes a new height-0 peak, then merges the last two peaks
/// on the stack while they share a height — mirroring the carries you get
/// incrementing a binary counter. Because merges are decided from the
/// peaks actually on hand rather than a position formula, there's no way
/// for an unrelated node to be mistaken for a sibling.
pub struct MerkleRangeTreeV2 {
    /// Storage for all nodes ever created, keyed by postorder position, kept for debugging.
    nodes: BTreeMap<u64, (Sha256Hash, u32, String)>,
    /// Current peaks, left to right, each strictly taller than the one before it.
    peaks: Vec<(u64, u32, Sha256Hash)>,
    /// Current size (number of leaves)
    size: u64,
    /// Next free postorder position, i.e. total nodes (leaves + merges) ever inserted
    next_pos: u64,
}

impl MerkleRangeTreeV2 {
    /// Create a new empty MMR
    pub fn new() -> Self {
        Self {
            nodes: BTreeMap::new(),
            peaks: Vec::new(),
            size: 0,
            next_pos: 0,
        }
    }

    /// Append a new leaf to the MMR
    pub fn append(&mut self, value: String) -> (u64, u32) {
        let hash: Sha256Hash = value.as_str().into();
        let pos = self.next_pos;
        self.next_pos += 1;
        self.nodes.insert(pos, (hash, 0, value));
        self.size += 1;
        self.peaks.push((pos, 0, hash));

        let mut height = 0;
        while self.peaks.len() >= 2 {
            let (right_pos, right_height, right_hash) = self.peaks[self.peaks.len() - 1];
            let (left_pos, left_height, left_hash) = self.peaks[self.peaks.len() - 2];
            if left_height != right_height {
                break;
            }

            self.peaks.pop();
            self.peaks.pop();

            let parent_hash = left_hash + right_hash;
            let parent_height = left_height + 1;
            let parent_pos = self.next_pos;
            self.next_pos += 1;
            self.nodes.insert(
                parent_pos,
                (parent_hash, parent_height, format!("{left_hash}+{right_hash}")),
            );
            self.peaks.push((parent_pos, parent_height, parent_hash));
            height = parent_height;
        }

        (pos, height)
    }

    /// Get the peak positions for the current MMR
    pub fn get_peaks(&self) -> Vec<u64> {
        self.peaks.iter().map(|&(pos, _, _)| pos).collect()
    }

    /// Calculate the root hash by bagging all peaks
    pub fn get_root(&self) -> Option<Sha256Hash> {
        let mut peak_hashes: Vec<Sha256Hash> = self.peaks.iter().map(|&(_, _, hash)| hash).collect();

        if peak_hashes.is_empty() {
            return None;
        }

        // Bag the peaks from right to left
        while peak_hashes.len() > 1 {
            let right = peak_hashes.pop().unwrap();
            let left = peak_hashes.pop().unwrap();
            peak_hashes.push(left + right);
        }

        peak_hashes.into_iter().next()
    }

    /// Get the number of leaves in the MMR
    pub fn len(&self) -> u64 {
        self.size
    }

    pub fn nodes(&self) -> usize {
        self.nodes.len()
    }

    /// Check if the MMR is empty
    pub fn is_empty(&self) -> bool {
        self.size == 0
    }
}

impl Debug for MerkleRangeTreeV2 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let header = format!("MerkleRangeTreeV2 {{ size: {}", self.size);
        let nodes = self
            .nodes
            .iter()
            .map(|(k, (h, height, v))| format!("{}, h{}, {}, {}\n", k, height, h, v))
            .collect::<Vec<String>>()
            .join("");
        let peaks = format!("peaks: {:?}", self.get_peaks());
        write!(f, "{}\n{}{}\n }}", header, nodes, peaks)
    }
}

#[cfg(test)]
mod tests {
    use crate::mmr_v2::MerkleRangeTreeV2;

    #[test]
    pub fn test() {
        let mut mmr = MerkleRangeTreeV2::new();
        assert_eq!(mmr.len(), 0);
        assert!(mmr.is_empty());

        let (pos, height) = mmr.append("a".into());
        dbg!(&mmr);
        assert_eq!(pos, 0);
        assert_eq!(height, 0);
        assert_eq!(mmr.len(), 1);
        assert_eq!(&mmr.get_peaks(), &[0]);

        let (pos, height) = mmr.append("b".into());
        dbg!(&mmr);
        assert_eq!(pos, 1);
        assert_eq!(height, 1);
        assert_eq!(mmr.len(), 2);
        assert_eq!(mmr.nodes(), 3);
        assert_eq!(&mmr.get_peaks(), &[2]);

        let (pos, height) = mmr.append("c".into());
        dbg!(&mmr);
        assert_eq!(pos, 3, "pos(3)");
        assert_eq!(height, 0, "h(3)");
        assert_eq!(mmr.len(), 3);
        assert_eq!(mmr.nodes(), 4);
        assert_eq!(&mmr.get_peaks(), &[2, 3]);

        let (pos, height) = mmr.append("d".into());
        dbg!(&mmr);
        assert_eq!(pos, 4, "pos(4)");
        assert_eq!(height, 2, "h(4)");
        assert_eq!(mmr.len(), 4);
        assert_eq!(mmr.nodes(), 7);
        assert_eq!(&mmr.get_peaks(), &[6]);

        // Sanity check: both implementations must agree on the root.
        let mut v1 = crate::mmr::MerkleRangeTree::new();
        v1.append("a".into());
        v1.append("b".into());
        v1.append("c".into());
        v1.append("d".into());
        assert_eq!(mmr.get_root(), v1.get_root());
    }
}
