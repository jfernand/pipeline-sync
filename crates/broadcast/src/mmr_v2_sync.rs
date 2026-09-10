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
