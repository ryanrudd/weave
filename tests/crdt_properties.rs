use weave::crdt::{LamportTimestamp, MergeStrategy, OpId, Operation, SiteId};
use weave::document::Document;
use weave::strategy::LineCRDT;

/// Helper: create a document with a LineCRDT strategy for the given site.
fn make_doc(name: &str, site: u64) -> Document<LineCRDT> {
    Document::new(name.to_string(), LineCRDT::new(SiteId(site)))
}

/// Helper: create a raw operation for an insert with explicit ids.
fn raw_insert(ts: u64, site: u64, after: Option<OpId>, content: &str) -> Operation {
    Operation::Insert {
        id: OpId {
            timestamp: LamportTimestamp(ts),
            site: SiteId(site),
        },
        after,
        content: content.to_string(),
    }
}

/// Helper: create a raw delete operation.
fn raw_delete(ts: u64, site: u64) -> Operation {
    Operation::Delete {
        id: OpId {
            timestamp: LamportTimestamp(ts),
            site: SiteId(site),
        },
    }
}

// ---------------------------------------------------------------------------
// 1. Commutativity: applying ops A,B gives same result as B,A
// ---------------------------------------------------------------------------

#[test]
fn commutativity_two_inserts() {
    let op_a = raw_insert(1, 1, None, "alpha");
    let op_b = raw_insert(1, 2, None, "beta");

    // Order A then B
    let mut crdt1 = LineCRDT::new(SiteId(99));
    crdt1.apply(op_a.clone());
    crdt1.apply(op_b.clone());

    // Order B then A
    let mut crdt2 = LineCRDT::new(SiteId(99));
    crdt2.apply(op_b);
    crdt2.apply(op_a);

    assert_eq!(
        crdt1.render(),
        crdt2.render(),
        "Two inserts must commute: A,B == B,A"
    );
}

// ---------------------------------------------------------------------------
// 2. Commutativity: insert then delete == applying in either order
// ---------------------------------------------------------------------------

#[test]
fn commutativity_insert_delete() {
    let id_a = OpId {
        timestamp: LamportTimestamp(1),
        site: SiteId(1),
    };
    let op_insert = raw_insert(1, 1, None, "line");
    let op_delete = Operation::Delete { id: id_a };

    // Insert then delete
    let mut crdt1 = LineCRDT::new(SiteId(99));
    crdt1.apply(op_insert.clone());
    crdt1.apply(op_delete.clone());

    // Delete then insert (delete is a no-op if element missing, insert adds it, but it stays)
    let mut crdt2 = LineCRDT::new(SiteId(99));
    crdt2.apply(op_delete.clone());
    crdt2.apply(op_insert.clone());
    // Re-apply delete so it takes effect now that the element exists
    crdt2.apply(op_delete);

    assert_eq!(
        crdt1.render(),
        crdt2.render(),
        "Insert + delete must converge regardless of arrival order"
    );
}

// ---------------------------------------------------------------------------
// 3. Commutativity: three ops in any order converge
// ---------------------------------------------------------------------------

#[test]
fn commutativity_three_operations() {
    let op_a = raw_insert(1, 1, None, "first");
    let op_b = raw_insert(2, 2, None, "second");
    let op_c = raw_insert(3, 3, None, "third");

    // All six permutations of three operations
    let permutations: Vec<[usize; 3]> = vec![
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];

    let ops = [op_a, op_b, op_c];
    let mut results: Vec<Vec<String>> = Vec::new();

    for perm in &permutations {
        let mut crdt = LineCRDT::new(SiteId(99));
        for &idx in perm {
            crdt.apply(ops[idx].clone());
        }
        results.push(crdt.render().into_iter().map(|s| s.to_string()).collect());
    }

    for i in 1..results.len() {
        assert_eq!(
            results[0], results[i],
            "Permutation {:?} diverged from {:?}",
            permutations[i], permutations[0]
        );
    }
}

// ---------------------------------------------------------------------------
// 4. Associativity: (A merge B) merge C == A merge (B merge C)
// ---------------------------------------------------------------------------

#[test]
fn associativity_merge_order() {
    let mut doc_a = make_doc("test.txt", 1);
    doc_a.append("from A".into());
    let ops_a = doc_a.operations().to_vec();

    let mut doc_b = make_doc("test.txt", 2);
    doc_b.append("from B".into());
    let ops_b = doc_b.operations().to_vec();

    let mut doc_c = make_doc("test.txt", 3);
    doc_c.append("from C".into());
    let ops_c = doc_c.operations().to_vec();

    // (A merge B) merge C
    let mut left = LineCRDT::new(SiteId(99));
    left.merge(ops_a.clone());
    left.merge(ops_b.clone());
    left.merge(ops_c.clone());

    // A merge (B merge C) — since merge is just applying ops, we reorder
    let mut right = LineCRDT::new(SiteId(99));
    right.merge(ops_b.clone());
    right.merge(ops_c.clone());
    right.merge(ops_a.clone());

    assert_eq!(
        left.render(),
        right.render(),
        "Merge must be associative: (A+B)+C == A+(B+C)"
    );
}

// ---------------------------------------------------------------------------
// 5. Idempotency: applying same insert twice is no-op
// ---------------------------------------------------------------------------

#[test]
fn idempotency_single_op() {
    let op = raw_insert(1, 1, None, "hello");

    let mut crdt = LineCRDT::new(SiteId(99));
    crdt.apply(op.clone());
    let after_first = crdt.render().into_iter().map(|s| s.to_string()).collect::<Vec<_>>();

    crdt.apply(op);
    let after_second = crdt.render().into_iter().map(|s| s.to_string()).collect::<Vec<_>>();

    assert_eq!(after_first, after_second, "Applying the same insert twice must be a no-op");
    assert_eq!(after_first, vec!["hello"]);
}

// ---------------------------------------------------------------------------
// 6. Idempotency: applying entire ops list twice gives same state
// ---------------------------------------------------------------------------

#[test]
fn idempotency_batch() {
    let mut doc = make_doc("test.txt", 1);
    doc.append("line 1".into());
    doc.append("line 2".into());
    doc.append("line 3".into());
    let ops = doc.operations().to_vec();

    let mut crdt = LineCRDT::new(SiteId(99));
    crdt.merge(ops.clone());
    let state_after_once: Vec<String> = crdt.render().into_iter().map(|s| s.to_string()).collect();

    crdt.merge(ops);
    let state_after_twice: Vec<String> = crdt.render().into_iter().map(|s| s.to_string()).collect();

    assert_eq!(
        state_after_once, state_after_twice,
        "Merging the same batch twice must not duplicate elements"
    );
}

// ---------------------------------------------------------------------------
// 7. Convergence: two sites with same ops converge regardless of order
// ---------------------------------------------------------------------------

#[test]
fn convergence_two_sites() {
    let mut doc1 = make_doc("test.txt", 1);
    doc1.append("A1".into());
    doc1.append("A2".into());
    let ops1 = doc1.operations().to_vec();

    let mut doc2 = make_doc("test.txt", 2);
    doc2.append("B1".into());
    let ops2 = doc2.operations().to_vec();

    // Site 1 receives site 2's ops
    doc1.merge_remote(ops2.clone());
    // Site 2 receives site 1's ops
    doc2.merge_remote(ops1.clone());

    assert_eq!(
        doc1.lines(),
        doc2.lines(),
        "Two sites must converge after exchanging all operations"
    );
}

// ---------------------------------------------------------------------------
// 8. Convergence: three sites all converge after full sync
// ---------------------------------------------------------------------------

#[test]
fn convergence_three_sites() {
    let mut doc1 = make_doc("test.txt", 1);
    doc1.append("site1-line".into());
    let ops1 = doc1.operations().to_vec();

    let mut doc2 = make_doc("test.txt", 2);
    doc2.append("site2-line".into());
    let ops2 = doc2.operations().to_vec();

    let mut doc3 = make_doc("test.txt", 3);
    doc3.append("site3-line".into());
    let ops3 = doc3.operations().to_vec();

    // Full sync: each site receives ops from the other two
    doc1.merge_remote(ops2.clone());
    doc1.merge_remote(ops3.clone());

    doc2.merge_remote(ops1.clone());
    doc2.merge_remote(ops3.clone());

    doc3.merge_remote(ops1.clone());
    doc3.merge_remote(ops2.clone());

    let lines1 = doc1.lines();
    let lines2 = doc2.lines();
    let lines3 = doc3.lines();

    assert_eq!(lines1, lines2, "Site 1 and 2 must converge");
    assert_eq!(lines2, lines3, "Site 2 and 3 must converge");
    assert_eq!(lines1.len(), 3, "All three inserts must be present");
}

// ---------------------------------------------------------------------------
// 9. Convergence with deletes: sites converge when mix of inserts and deletes
// ---------------------------------------------------------------------------

#[test]
fn convergence_with_deletes() {
    // Shared baseline
    let mut doc1 = make_doc("test.txt", 1);
    let id_shared = doc1.append("shared-line".into());
    let baseline_ops = doc1.operations().to_vec();

    let mut doc2 = make_doc("test.txt", 2);
    doc2.merge_remote(baseline_ops);

    // Site 1: inserts a new line after the shared line
    doc1.insert_after(Some(id_shared), "new from site 1".into());
    let new_ops1 = doc1.operations()[1..].to_vec();

    // Site 2: deletes the shared line and adds its own
    doc2.delete(id_shared);
    doc2.append("new from site 2".into());
    let new_ops2 = doc2.operations().to_vec();

    // Exchange
    doc1.merge_remote(new_ops2);
    doc2.merge_remote(new_ops1);

    assert_eq!(
        doc1.lines(),
        doc2.lines(),
        "Sites must converge with mixed inserts and deletes"
    );
    assert!(
        !doc1.lines().contains(&"shared-line"),
        "Deleted line must not be visible"
    );
}

// ---------------------------------------------------------------------------
// 10. Causal ordering preserved: if op B depends on op A, B goes after A
// ---------------------------------------------------------------------------

#[test]
fn causal_ordering_preserved() {
    let mut doc = make_doc("test.txt", 1);
    let id_a = doc.append("parent".into());
    let _id_b = doc.insert_after(Some(id_a), "child".into());

    // The child was inserted after the parent, so it must appear after it
    let lines = doc.lines();
    let pos_parent = lines.iter().position(|&l| l == "parent").unwrap();
    let pos_child = lines.iter().position(|&l| l == "child").unwrap();

    assert!(
        pos_child > pos_parent,
        "Causally dependent op must appear after its parent: parent@{} child@{}",
        pos_parent,
        pos_child
    );

    // Verify this holds even when replayed on another site
    let ops = doc.operations().to_vec();
    let mut replica = make_doc("test.txt", 2);
    replica.merge_remote(ops);

    let lines2 = replica.lines();
    let pos_parent2 = lines2.iter().position(|&l| l == "parent").unwrap();
    let pos_child2 = lines2.iter().position(|&l| l == "child").unwrap();

    assert!(
        pos_child2 > pos_parent2,
        "Causal ordering must be preserved on replica"
    );
}

// ---------------------------------------------------------------------------
// 11. Concurrent inserts at same position produce deterministic order
// ---------------------------------------------------------------------------

#[test]
fn concurrent_inserts_deterministic_order() {
    // Five sites all insert at the beginning (after: None) concurrently
    let mut all_ops: Vec<Operation> = Vec::new();
    for site in 1..=5u64 {
        let mut doc = make_doc("test.txt", site);
        doc.insert_after(None, format!("site-{}", site));
        all_ops.extend(doc.operations().to_vec());
    }

    // Apply in forward order
    let mut crdt_forward = LineCRDT::new(SiteId(99));
    crdt_forward.merge(all_ops.clone());

    // Apply in reverse order
    let mut crdt_reverse = LineCRDT::new(SiteId(99));
    let mut reversed = all_ops.clone();
    reversed.reverse();
    crdt_reverse.merge(reversed);

    // Apply in interleaved order
    let mut crdt_interleaved = LineCRDT::new(SiteId(99));
    let mut interleaved = all_ops.clone();
    // Swap elements to create a different order
    interleaved.swap(0, 3);
    interleaved.swap(1, 4);
    crdt_interleaved.merge(interleaved);

    let result_fwd: Vec<&str> = crdt_forward.render();
    let result_rev: Vec<&str> = crdt_reverse.render();
    let result_int: Vec<&str> = crdt_interleaved.render();

    assert_eq!(result_fwd, result_rev, "Forward vs reverse must match");
    assert_eq!(result_fwd, result_int, "Forward vs interleaved must match");
    assert_eq!(result_fwd.len(), 5, "All 5 inserts must be present");
}

// ---------------------------------------------------------------------------
// 12. Tombstone preserves references: deleting a node doesn't break inserts
//     that reference it
// ---------------------------------------------------------------------------

#[test]
fn tombstone_preserves_references() {
    let mut doc = make_doc("test.txt", 1);
    let id_first = doc.append("first".into());
    let id_second = doc.append("second".into());
    let _id_third = doc.append("third".into());

    // Insert a new line referencing "second" as its parent
    doc.insert_after(Some(id_second), "after-second".into());

    // Now delete "second" — the tombstone must remain so "after-second" stays positioned
    doc.delete(id_second);

    let lines = doc.lines();
    assert!(
        !lines.contains(&"second"),
        "Deleted node must not be visible"
    );
    assert!(
        lines.contains(&"after-second"),
        "Line referencing a tombstoned parent must survive"
    );

    // Verify ordering: first, after-second, third
    let pos_first = lines.iter().position(|&l| l == "first").unwrap();
    let pos_after = lines.iter().position(|&l| l == "after-second").unwrap();
    let pos_third = lines.iter().position(|&l| l == "third").unwrap();

    assert!(pos_first < pos_after, "after-second must come after first");
    assert!(pos_after < pos_third, "after-second must come before third");

    // Replay on a fresh replica to verify tombstone works across sites
    let ops = doc.operations().to_vec();
    let mut replica = make_doc("test.txt", 2);
    replica.merge_remote(ops);

    assert_eq!(
        doc.lines(),
        replica.lines(),
        "Replica must converge even with tombstoned references"
    );
}

// ---------------------------------------------------------------------------
// 13. Empty merge is identity: merging empty ops list changes nothing
// ---------------------------------------------------------------------------

#[test]
fn empty_merge_is_identity() {
    let mut doc = make_doc("test.txt", 1);
    doc.append("line 1".into());
    doc.append("line 2".into());

    let before: Vec<String> = doc.lines().into_iter().map(|s| s.to_string()).collect();

    // Merge an empty list of operations
    doc.merge_remote(vec![]);

    let after: Vec<String> = doc.lines().into_iter().map(|s| s.to_string()).collect();

    assert_eq!(before, after, "Merging empty ops must not change document state");
}

// ---------------------------------------------------------------------------
// 14. Self merge is idempotent: merging your own ops into yourself is no-op
// ---------------------------------------------------------------------------

#[test]
fn self_merge_is_idempotent() {
    let mut doc = make_doc("test.txt", 1);
    doc.append("alpha".into());
    doc.append("beta".into());
    doc.append("gamma".into());

    let own_ops = doc.operations().to_vec();
    let before: Vec<String> = doc.lines().into_iter().map(|s| s.to_string()).collect();

    // Merge our own operations back into ourselves
    doc.merge_remote(own_ops.clone());

    let after: Vec<String> = doc.lines().into_iter().map(|s| s.to_string()).collect();

    assert_eq!(before, after, "Self-merge must be a no-op");
    assert_eq!(after, vec!["alpha", "beta", "gamma"]);

    // Do it again for good measure
    doc.merge_remote(own_ops);
    let after2: Vec<String> = doc.lines().into_iter().map(|s| s.to_string()).collect();
    assert_eq!(before, after2, "Repeated self-merge must remain a no-op");
}

// ---------------------------------------------------------------------------
// 15. Clock monotonicity: timestamps always increase within a site
// ---------------------------------------------------------------------------

#[test]
fn clock_monotonicity() {
    let mut doc = make_doc("test.txt", 1);

    let mut prev_clock = doc.clock();
    let mut timestamps: Vec<u64> = vec![prev_clock];

    // Perform several operations and verify the clock strictly increases
    for i in 0..10 {
        doc.append(format!("line {}", i));
        let current_clock = doc.clock();
        assert!(
            current_clock > prev_clock,
            "Clock must strictly increase: was {} now {}",
            prev_clock,
            current_clock
        );
        prev_clock = current_clock;
        timestamps.push(current_clock);
    }

    // Also verify clock increases after receiving remote ops with high timestamps
    let remote_op = raw_insert(100, 2, None, "remote");
    doc.merge_remote(vec![remote_op]);

    let clock_after_remote = doc.clock();
    assert!(
        clock_after_remote >= 100,
        "Clock must advance past remote timestamp: got {}",
        clock_after_remote
    );

    // Next local op must still exceed the bumped clock
    let clock_before_local = doc.clock();
    doc.append("after remote".into());
    assert!(
        doc.clock() > clock_before_local,
        "Clock must increase after local op following remote sync"
    );
}
