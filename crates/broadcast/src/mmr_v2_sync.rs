#![allow(unused)]
use crate::mmr_v2::MerkleRangeTreeV2;

/// Result of attempting to bring `local` up to date with `remote`.
#[derive(Debug, PartialEq)]
pub enum SyncOutcome {
    /// Roots already match; nothing to do.
    AlreadyInSync,
    /// `remote` was a genuine extension of `local`'s history; the missing
    /// leaves were appended to `local`.
    FastForwarded { leaves_applied: usize },
    /// `local` is already ahead of (or equal to, handled above) `remote`;
    /// `remote` is the one that needs to fast-forward, not `local`.
    RemoteBehind,
    /// Neither tree's history is a prefix of the other's — a real fork.
    /// Plain MMR peak/leaf data can't reconcile this; it needs an
    /// application-level merge (see `chain::EventChain::add_merge_event`).
    Diverged,
}

/// Bring `local` up to date with `remote` when possible, using only the
/// interrogation surface `MerkleRangeTreeV2` already exposes (peak_hashes,
/// node_hash, leaf). Two peers with a shared prefix converge without ever
/// transferring more than the missing leaves.
pub fn sync(local: &mut MerkleRangeTreeV2, remote: &MerkleRangeTreeV2) -> SyncOutcome {
    if local.get_root() == remote.get_root() {
        return SyncOutcome::AlreadyInSync;
    }

    if local.len() < remote.len() && is_prefix(local, remote) {
        let leaves = leaves_after(remote, local.len());
        let leaves_applied = leaves.len();
        for value in leaves {
            local.append(value);
        }
        SyncOutcome::FastForwarded { leaves_applied }
    } else if local.len() > remote.len() && is_prefix(remote, local) {
        SyncOutcome::RemoteBehind
    } else {
        SyncOutcome::Diverged
    }
}

/// Whether `shorter`'s entire history is reproduced inside `longer`: every
/// one of `shorter`'s peaks must appear in `longer` at the same position with
/// the same hash. Since MMR postorder positions are a pure function of the
/// append sequence, that's both necessary and sufficient for `shorter` to be
/// a genuine prefix of `longer` — a peak's hash already commits to its whole
/// subtree, so nothing earlier needs checking separately. Vacuously true when
/// `shorter` is empty.
fn is_prefix(shorter: &MerkleRangeTreeV2, longer: &MerkleRangeTreeV2) -> bool {
    shorter
        .peak_hashes()
        .into_iter()
        .all(|(pos, _height, hash)| longer.node_hash(pos) == Some(hash))
}

/// The leaf values `remote` has beyond the first `skip` leaves, in original
/// append order. Positions are scanned in order because postorder assigns
/// each leaf its position at append time, before any later merges, so
/// leaf-only positions already appear in append order among the merge nodes.
fn leaves_after(remote: &MerkleRangeTreeV2, skip: u64) -> Vec<String> {
    let mut leaves = Vec::new();
    let mut seen = 0u64;
    for pos in 0..remote.nodes() as u64 {
        if let Some(value) = remote.leaf(pos) {
            if seen >= skip {
                leaves.push(value);
            }
            seen += 1;
        }
    }
    leaves
}

#[cfg(test)]
mod tests {
    use super::{sync, SyncOutcome};
    use crate::mmr_v2::MerkleRangeTreeV2;

    fn tree(values: &[&str]) -> MerkleRangeTreeV2 {
        let mut tree = MerkleRangeTreeV2::new();
        for value in values {
            tree.append((*value).to_string());
        }
        tree
    }

    #[test]
    fn both_empty_are_already_in_sync() {
        let mut local = tree(&[]);
        let remote = tree(&[]);
        assert_eq!(sync(&mut local, &remote), SyncOutcome::AlreadyInSync);
        assert_eq!(local.len(), 0);
    }

    #[test]
    fn identical_trees_are_already_in_sync() {
        let mut local = tree(&["a", "b", "c", "d"]);
        let remote = tree(&["a", "b", "c", "d"]);
        assert_eq!(sync(&mut local, &remote), SyncOutcome::AlreadyInSync);
        assert_eq!(local.len(), 4);
    }

    #[test]
    fn empty_local_fast_forwards_across_a_merge_boundary() {
        let mut local = tree(&[]);
        let remote = tree(&["a", "b", "c", "d"]); // crosses the 2-leaf and 4-leaf merges
        assert_eq!(
            sync(&mut local, &remote),
            SyncOutcome::FastForwarded { leaves_applied: 4 }
        );
        assert_eq!(local.get_root(), remote.get_root());
        assert_eq!(local.len(), remote.len());
    }

    #[test]
    fn partial_prefix_fast_forwards_only_the_missing_leaves() {
        let mut local = tree(&["a", "b"]);
        let remote = tree(&["a", "b", "c", "d"]);
        assert_eq!(
            sync(&mut local, &remote),
            SyncOutcome::FastForwarded { leaves_applied: 2 }
        );
        assert_eq!(local.get_root(), remote.get_root());
    }

    #[test]
    fn fast_forward_checks_every_peak_when_local_has_several() {
        // local (3 leaves) has two peaks: a+b, and c. Both must be validated
        // against remote, not just the most recent one.
        let mut local = tree(&["a", "b", "c"]);
        let remote = tree(&["a", "b", "c", "d", "e", "f", "g"]);
        assert_eq!(
            sync(&mut local, &remote),
            SyncOutcome::FastForwarded { leaves_applied: 4 }
        );
        assert_eq!(local.get_root(), remote.get_root());
    }

    #[test]
    fn local_ahead_of_remote_reports_remote_behind() {
        let mut local = tree(&["a", "b", "c", "d"]);
        let remote = tree(&["a", "b"]);
        assert_eq!(sync(&mut local, &remote), SyncOutcome::RemoteBehind);
        // local is untouched — it's remote's job to fast-forward, not local's.
        assert_eq!(local.len(), 4);
    }

    #[test]
    fn divergence_at_the_first_leaf_is_detected() {
        let mut local = tree(&["a", "b"]);
        let remote = tree(&["x", "b"]);
        assert_eq!(sync(&mut local, &remote), SyncOutcome::Diverged);
        assert_eq!(local.len(), 2, "a diverged tree must not be mutated");
    }

    #[test]
    fn divergence_in_only_the_last_leaf_is_still_detected() {
        // Same size; the a+b peak matches but the standalone `c` peak doesn't.
        // A naive "any peak matches" check would wrongly call this in sync.
        let mut local = tree(&["a", "b", "c"]);
        let remote = tree(&["a", "b", "c2"]);
        assert_eq!(sync(&mut local, &remote), SyncOutcome::Diverged);
        assert_eq!(local.len(), 3, "a diverged tree must not be mutated");
    }

    #[test]
    fn equal_size_with_matching_content_is_in_sync_not_diverged() {
        let mut local = tree(&["a", "b", "c"]);
        let remote = tree(&["a", "b", "c"]);
        assert_eq!(sync(&mut local, &remote), SyncOutcome::AlreadyInSync);
    }

    #[test]
    fn syncing_every_size_up_to_a_full_binary_carry_converges() {
        // Exercise every leaf-count boundary from 0 to 16 (crossing every
        // power-of-two merge cascade at least once) and confirm a fresh
        // local tree always converges to remote's exact root.
        let values: Vec<String> = (0..16).map(|i| format!("leaf-{i}")).collect();
        for n in 0..=values.len() {
            let remote = tree(
                &values[..n]
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
            );
            let mut local = MerkleRangeTreeV2::new();
            let outcome = sync(&mut local, &remote);
            if n == 0 {
                assert_eq!(outcome, SyncOutcome::AlreadyInSync);
            } else {
                assert_eq!(outcome, SyncOutcome::FastForwarded { leaves_applied: n });
            }
            assert_eq!(local.get_root(), remote.get_root(), "n = {n}");
            assert_eq!(local.len(), remote.len(), "n = {n}");
        }
    }
}
