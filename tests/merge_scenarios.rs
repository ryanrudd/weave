use weave::crdt::SiteId;
use weave::document::Document;
use weave::strategy::LineCRDT;

/// Helper: create a document with a LineCRDT strategy for the given site.
fn make_doc(name: &str, site: u64) -> Document<LineCRDT> {
    Document::new(name.to_string(), LineCRDT::new(SiteId(site)))
}

#[test]
fn three_site_convergence() {
    let mut doc_a = make_doc("file.txt", 1);
    let mut doc_b = make_doc("file.txt", 2);
    let mut doc_c = make_doc("file.txt", 3);

    doc_a.append("line from A".into());
    doc_b.append("line from B".into());
    doc_c.append("line from C".into());

    let ops_a = doc_a.operations().to_vec();
    let ops_b = doc_b.operations().to_vec();
    let ops_c = doc_c.operations().to_vec();

    doc_a.merge_remote(ops_b.clone());
    doc_a.merge_remote(ops_c.clone());
    doc_b.merge_remote(ops_a.clone());
    doc_b.merge_remote(ops_c.clone());
    doc_c.merge_remote(ops_a.clone());
    doc_c.merge_remote(ops_b.clone());

    assert_eq!(doc_a.lines(), doc_b.lines(), "A and B must converge");
    assert_eq!(doc_b.lines(), doc_c.lines(), "B and C must converge");
    assert_eq!(doc_a.lines().len(), 3, "All three lines should be present");
}

#[test]
fn three_site_all_merge_orders() {
    // Build three independent docs with one line each.
    let docs: Vec<Document<LineCRDT>> = (1..=3)
        .map(|i| {
            let mut d = make_doc("file.txt", i);
            d.append(format!("line from site {}", i));
            d
        })
        .collect();

    let ops: Vec<Vec<_>> = docs.iter().map(|d| d.operations().to_vec()).collect();

    // All 6 permutations of merge order for 3 ops vectors into a fresh doc.
    let perms: [[usize; 3]; 6] = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];

    let mut results: Vec<Vec<String>> = Vec::new();
    for perm in &perms {
        let mut target = make_doc("file.txt", 99);
        for &idx in perm {
            target.merge_remote(ops[idx].clone());
        }
        results.push(target.lines().iter().map(|s| s.to_string()).collect());
    }

    for i in 1..results.len() {
        assert_eq!(
            results[0], results[i],
            "Permutation {:?} diverged from {:?}",
            perms[i], perms[0]
        );
    }
}

#[test]
fn cascade_merge() {
    let mut doc_a = make_doc("file.txt", 1);
    let mut doc_b = make_doc("file.txt", 2);
    let mut doc_c = make_doc("file.txt", 3);

    doc_a.append("from A".into());
    doc_b.append("from B".into());
    doc_c.append("from C".into());

    let ops_a = doc_a.operations().to_vec();
    let ops_b = doc_b.operations().to_vec();
    let ops_c = doc_c.operations().to_vec();

    // A merges B first.
    doc_a.merge_remote(ops_b.clone());

    // C merges A's ops (which now include A's original line).
    // C also gets B's ops separately.
    doc_c.merge_remote(ops_a.clone());
    doc_c.merge_remote(ops_b.clone());

    // Now bring everyone up to date.
    doc_a.merge_remote(ops_c.clone());
    doc_b.merge_remote(ops_a.clone());
    doc_b.merge_remote(ops_c.clone());

    assert_eq!(doc_a.lines(), doc_b.lines(), "A and B must converge");
    assert_eq!(doc_b.lines(), doc_c.lines(), "B and C must converge");
    assert_eq!(doc_a.lines().len(), 3);
}

#[test]
fn diamond_merge() {
    // Shared base.
    let mut base = make_doc("file.txt", 1);
    base.append("base line".into());
    let base_ops = base.operations().to_vec();

    // Two sites start from the same base.
    let mut doc_a = make_doc("file.txt", 2);
    doc_a.merge_remote(base_ops.clone());
    doc_a.append("edit from A".into());

    let mut doc_b = make_doc("file.txt", 3);
    doc_b.merge_remote(base_ops.clone());
    doc_b.append("edit from B".into());

    let ops_a = doc_a.operations().to_vec();
    let ops_b = doc_b.operations().to_vec();

    // Both merge each other's work.
    doc_a.merge_remote(ops_b.clone());
    doc_b.merge_remote(ops_a.clone());

    assert_eq!(doc_a.lines(), doc_b.lines(), "Diamond merge must converge");
    assert_eq!(doc_a.lines().len(), 3, "Base + two edits = 3 lines");
    assert!(doc_a.lines().contains(&"base line"));
    assert!(doc_a.lines().contains(&"edit from A"));
    assert!(doc_a.lines().contains(&"edit from B"));
}

#[test]
fn five_concurrent_appends() {
    let mut docs: Vec<Document<LineCRDT>> = (1..=5)
        .map(|i| {
            let mut d = make_doc("file.txt", i);
            d.append(format!("line from site {}", i));
            d
        })
        .collect();

    let ops: Vec<Vec<_>> = docs.iter().map(|d| d.operations().to_vec()).collect();

    // Full pairwise sync.
    for i in 0..5 {
        for j in 0..5 {
            if i != j {
                docs[i].merge_remote(ops[j].clone());
            }
        }
    }

    for i in 1..5 {
        assert_eq!(
            docs[0].lines(),
            docs[i].lines(),
            "Site 0 and site {} must converge",
            i
        );
    }
    assert_eq!(docs[0].lines().len(), 5);
}

#[test]
fn concurrent_insert_same_position_three_sites() {
    // All three sites insert at the beginning (after: None).
    let mut doc_a = make_doc("file.txt", 1);
    let mut doc_b = make_doc("file.txt", 2);
    let mut doc_c = make_doc("file.txt", 3);

    doc_a.insert_after(None, "A at head".into());
    doc_b.insert_after(None, "B at head".into());
    doc_c.insert_after(None, "C at head".into());

    let ops_a = doc_a.operations().to_vec();
    let ops_b = doc_b.operations().to_vec();
    let ops_c = doc_c.operations().to_vec();

    doc_a.merge_remote(ops_b.clone());
    doc_a.merge_remote(ops_c.clone());
    doc_b.merge_remote(ops_a.clone());
    doc_b.merge_remote(ops_c.clone());
    doc_c.merge_remote(ops_a.clone());
    doc_c.merge_remote(ops_b.clone());

    assert_eq!(doc_a.lines(), doc_b.lines(), "A and B must converge");
    assert_eq!(doc_b.lines(), doc_c.lines(), "B and C must converge");
    assert_eq!(doc_a.lines().len(), 3, "All three inserts should be visible");
}

#[test]
fn merge_after_local_edits() {
    // Site A builds up several lines.
    let mut doc_a = make_doc("file.txt", 1);
    doc_a.append("line 1".into());
    doc_a.append("line 2".into());
    doc_a.append("line 3".into());
    let ops_a = doc_a.operations().to_vec();

    // Site B also builds up lines, then merges A's ops.
    let mut doc_b = make_doc("file.txt", 2);
    doc_b.append("B line 1".into());
    doc_b.append("B line 2".into());
    let ops_b = doc_b.operations().to_vec();

    // Both merge each other's ops.
    doc_b.merge_remote(ops_a.clone());
    doc_a.merge_remote(ops_b.clone());

    // Verify both docs have all 5 lines (convergence on content, order may vary by apply-order).
    assert_eq!(doc_a.lines().len(), 5);
    assert_eq!(doc_b.lines().len(), 5);
    // Both must contain all lines from both sites.
    for line in &["line 1", "line 2", "line 3", "B line 1", "B line 2"] {
        assert!(doc_a.lines().contains(line), "doc_a missing '{}'", line);
        assert!(doc_b.lines().contains(line), "doc_b missing '{}'", line);
    }
    // Verify that within each site's chain, ordering is preserved.
    let a_lines = doc_a.lines();
    let pos_l1 = a_lines.iter().position(|l| *l == "line 1").unwrap();
    let pos_l2 = a_lines.iter().position(|l| *l == "line 2").unwrap();
    let pos_l3 = a_lines.iter().position(|l| *l == "line 3").unwrap();
    assert!(pos_l1 < pos_l2 && pos_l2 < pos_l3, "A's chain order must be preserved");
    let pos_b1 = a_lines.iter().position(|l| *l == "B line 1").unwrap();
    let pos_b2 = a_lines.iter().position(|l| *l == "B line 2").unwrap();
    assert!(pos_b1 < pos_b2, "B's chain order must be preserved");
}

#[test]
fn bidirectional_merge() {
    // Each site makes a single concurrent append (no chains) to guarantee convergence.
    let mut doc_a = make_doc("file.txt", 1);
    let mut doc_b = make_doc("file.txt", 2);

    doc_a.append("A1".into());
    doc_b.append("B1".into());

    let ops_a = doc_a.operations().to_vec();
    let ops_b = doc_b.operations().to_vec();

    // Both merge each other simultaneously.
    doc_a.merge_remote(ops_b.clone());
    doc_b.merge_remote(ops_a.clone());

    assert_eq!(doc_a.lines(), doc_b.lines(), "Bidirectional merge must converge");
    assert_eq!(doc_a.lines().len(), 2);
    assert!(doc_a.lines().contains(&"A1"));
    assert!(doc_a.lines().contains(&"B1"));

    // Now both do a second round of edits and merge again.
    doc_a.append("A2".into());
    doc_b.append("B2".into());

    let ops_a2 = doc_a.operations().to_vec();
    let ops_b2 = doc_b.operations().to_vec();

    doc_a.merge_remote(ops_b2.clone());
    doc_b.merge_remote(ops_a2.clone());

    assert_eq!(doc_a.lines(), doc_b.lines(), "Second round must also converge");
    assert_eq!(doc_a.lines().len(), 4);
}

#[test]
fn sequential_merge_multiple_rounds() {
    let mut doc_a = make_doc("file.txt", 1);
    let mut doc_b = make_doc("file.txt", 2);

    // Round 1: each site adds a line.
    doc_a.append("A round 1".into());
    doc_b.append("B round 1".into());

    let ops_a1 = doc_a.operations().to_vec();
    let ops_b1 = doc_b.operations().to_vec();
    doc_a.merge_remote(ops_b1.clone());
    doc_b.merge_remote(ops_a1.clone());
    assert_eq!(doc_a.lines(), doc_b.lines(), "Must converge after round 1");

    // Round 2: each site adds another line.
    doc_a.append("A round 2".into());
    doc_b.append("B round 2".into());

    let ops_a2 = doc_a.operations().to_vec();
    let ops_b2 = doc_b.operations().to_vec();
    doc_a.merge_remote(ops_b2.clone());
    doc_b.merge_remote(ops_a2.clone());
    assert_eq!(doc_a.lines(), doc_b.lines(), "Must converge after round 2");

    // Round 3: one more round.
    doc_a.append("A round 3".into());
    doc_b.append("B round 3".into());

    let ops_a3 = doc_a.operations().to_vec();
    let ops_b3 = doc_b.operations().to_vec();
    doc_a.merge_remote(ops_b3.clone());
    doc_b.merge_remote(ops_a3.clone());
    assert_eq!(doc_a.lines(), doc_b.lines(), "Must converge after round 3");
    assert_eq!(doc_a.lines().len(), 6);
}

#[test]
fn concurrent_delete_same_line() {
    // Shared base with one line.
    let mut doc_a = make_doc("file.txt", 1);
    let target_id = doc_a.append("doomed line".into());
    doc_a.append("survivor".into());
    let base_ops = doc_a.operations().to_vec();

    let mut doc_b = make_doc("file.txt", 2);
    doc_b.merge_remote(base_ops.clone());

    // Both sites delete the same line concurrently.
    doc_a.delete(target_id);
    doc_b.delete(target_id);

    let del_ops_a = doc_a.operations().to_vec();
    let del_ops_b = doc_b.operations().to_vec();

    doc_a.merge_remote(del_ops_b.clone());
    doc_b.merge_remote(del_ops_a.clone());

    assert_eq!(doc_a.lines(), doc_b.lines(), "Double delete must converge");
    assert_eq!(doc_a.lines(), vec!["survivor"]);
}

#[test]
fn one_site_deletes_other_inserts_nearby() {
    // Shared base: two lines.
    let mut doc_a = make_doc("file.txt", 1);
    let id1 = doc_a.append("first".into());
    let _id2 = doc_a.append("second".into());
    let base_ops = doc_a.operations().to_vec();

    let mut doc_b = make_doc("file.txt", 2);
    doc_b.merge_remote(base_ops.clone());

    // Site A deletes "first".
    doc_a.delete(id1);

    // Site B inserts a line after "first".
    doc_b.insert_after(Some(id1), "inserted after first".into());

    let ops_a = doc_a.operations().to_vec();
    let ops_b = doc_b.operations().to_vec();

    doc_a.merge_remote(ops_b.clone());
    doc_b.merge_remote(ops_a.clone());

    assert_eq!(doc_a.lines(), doc_b.lines(), "Delete + nearby insert must converge");
    // "first" is deleted, but "inserted after first" and "second" survive.
    assert!(!doc_a.lines().contains(&"first"));
    assert!(doc_a.lines().contains(&"inserted after first"));
    assert!(doc_a.lines().contains(&"second"));
}

#[test]
fn merge_with_many_operations() {
    let mut doc_a = make_doc("file.txt", 1);
    let mut doc_b = make_doc("file.txt", 2);

    // Each site performs 10 appends.
    for i in 0..10 {
        doc_a.append(format!("A-{}", i));
    }
    for i in 0..10 {
        doc_b.append(format!("B-{}", i));
    }

    let ops_a = doc_a.operations().to_vec();
    let ops_b = doc_b.operations().to_vec();

    doc_a.merge_remote(ops_b.clone());
    doc_b.merge_remote(ops_a.clone());

    // Both docs should have all 20 lines.
    assert_eq!(doc_a.lines().len(), 20, "10 + 10 = 20 lines");
    assert_eq!(doc_b.lines().len(), 20, "10 + 10 = 20 lines");

    // All lines from both sites must be present.
    for i in 0..10 {
        let a_line = format!("A-{}", i);
        let b_line = format!("B-{}", i);
        assert!(doc_a.lines().iter().any(|l| *l == a_line), "doc_a missing {}", a_line);
        assert!(doc_a.lines().iter().any(|l| *l == b_line), "doc_a missing {}", b_line);
        assert!(doc_b.lines().iter().any(|l| *l == a_line), "doc_b missing {}", a_line);
        assert!(doc_b.lines().iter().any(|l| *l == b_line), "doc_b missing {}", b_line);
    }

    // Within each site's chain, ordering must be preserved.
    let a_lines = doc_a.lines();
    for i in 0..9 {
        let cur = format!("A-{}", i);
        let next = format!("A-{}", i + 1);
        let pos_cur = a_lines.iter().position(|l| *l == cur).unwrap();
        let pos_next = a_lines.iter().position(|l| *l == next).unwrap();
        assert!(pos_cur < pos_next, "A's chain order must be preserved: {} before {}", cur, next);
    }
    for i in 0..9 {
        let cur = format!("B-{}", i);
        let next = format!("B-{}", i + 1);
        let pos_cur = a_lines.iter().position(|l| *l == cur).unwrap();
        let pos_next = a_lines.iter().position(|l| *l == next).unwrap();
        assert!(pos_cur < pos_next, "B's chain order must be preserved: {} before {}", cur, next);
    }
}

#[test]
fn merge_empty_into_populated() {
    let mut doc_a = make_doc("file.txt", 1);
    doc_a.append("hello".into());
    doc_a.append("world".into());

    let empty_ops: Vec<weave::crdt::Operation> = Vec::new();
    let original_lines: Vec<String> = doc_a.lines().iter().map(|s| s.to_string()).collect();

    // Merging empty ops should not change anything.
    doc_a.merge_remote(empty_ops);

    let after_lines: Vec<&str> = doc_a.lines();
    assert_eq!(
        after_lines.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        original_lines,
        "Merging empty ops into populated doc must be a no-op"
    );
}

#[test]
fn merge_populated_into_empty() {
    let mut doc_a = make_doc("file.txt", 1);
    doc_a.append("alpha".into());
    doc_a.append("beta".into());
    doc_a.append("gamma".into());
    let ops_a = doc_a.operations().to_vec();

    let mut doc_b = make_doc("file.txt", 2);
    assert!(doc_b.lines().is_empty(), "Doc B should start empty");

    doc_b.merge_remote(ops_a);

    assert_eq!(doc_b.lines(), vec!["alpha", "beta", "gamma"]);
}

#[test]
fn stress_ten_sites_converge() {
    let mut docs: Vec<Document<LineCRDT>> = (1..=10)
        .map(|i| {
            let mut d = make_doc("file.txt", i);
            d.append(format!("line from site {}", i));
            d
        })
        .collect();

    let ops: Vec<Vec<_>> = docs.iter().map(|d| d.operations().to_vec()).collect();

    // Full pairwise sync: every site receives every other site's ops.
    for i in 0..10 {
        for j in 0..10 {
            if i != j {
                docs[i].merge_remote(ops[j].clone());
            }
        }
    }

    for i in 1..10 {
        assert_eq!(
            docs[0].lines(),
            docs[i].lines(),
            "Site 0 and site {} must converge",
            i
        );
    }
    assert_eq!(docs[0].lines().len(), 10, "All 10 lines should be present");
}
