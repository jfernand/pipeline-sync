use broadcast::mmr_v2::MerkleRangeTreeV2;
use broadcast::mmr_v2_sync::{sync, SyncOutcome};

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
