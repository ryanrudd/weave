use weave::crdt::{LamportTimestamp, MergeStrategy, OpId, Operation, SiteId};
use weave::strategy::LineCRDT;

/// Helper: make an OpId from raw values.
fn op(ts: u64, site: u64) -> OpId {
    OpId {
        timestamp: LamportTimestamp(ts),
        site: SiteId(site),
    }
}

#[test]
fn empty_doc_render() {
    let crdt = LineCRDT::new(SiteId(1));
    let rendered = crdt.render();
    assert!(
        rendered.is_empty(),
        "Empty LineCRDT should render to empty vec"
    );
}

#[test]
fn single_line_insert_and_render() {
    let mut crdt = LineCRDT::new(SiteId(1));
    crdt.insert(None, "hello world".to_string());
    assert_eq!(crdt.render(), vec!["hello world"]);
}

#[test]
fn insert_at_beginning_of_nonempty() {
    let mut crdt = LineCRDT::new(SiteId(1));
    let first_op = crdt.insert(None, "second".to_string());
    let first_id = match &first_op {
        Operation::Insert { id, .. } => *id,
        _ => unreachable!(),
    };
    // Verify "second" is there
    assert_eq!(crdt.render(), vec!["second"]);

    // Insert at beginning (after=None)
    crdt.insert(None, "first".to_string());
    assert_eq!(crdt.render(), vec!["first", "second"]);

    // Verify the original element is still reachable
    assert!(crdt.visible_ids().contains(&first_id));
}

#[test]
fn insert_between_two_lines() {
    let mut crdt = LineCRDT::new(SiteId(1));
    let op_a = crdt.insert(None, "aaa".to_string());
    let id_a = match &op_a {
        Operation::Insert { id, .. } => *id,
        _ => unreachable!(),
    };
    crdt.insert(Some(id_a), "ccc".to_string());
    // Now insert between aaa and ccc
    crdt.insert(Some(id_a), "bbb".to_string());

    let lines = crdt.render();
    assert_eq!(lines, vec!["aaa", "bbb", "ccc"]);
}

#[test]
fn delete_only_element() {
    let mut crdt = LineCRDT::new(SiteId(1));
    let op = crdt.insert(None, "lonely".to_string());
    let id = match &op {
        Operation::Insert { id, .. } => *id,
        _ => unreachable!(),
    };
    assert_eq!(crdt.render().len(), 1);

    crdt.delete(id);
    assert!(
        crdt.render().is_empty(),
        "After deleting the only element, render should be empty"
    );
}

#[test]
fn delete_first_of_many() {
    let mut crdt = LineCRDT::new(SiteId(1));
    let op1 = crdt.insert(None, "first".to_string());
    let id1 = match &op1 {
        Operation::Insert { id, .. } => *id,
        _ => unreachable!(),
    };
    let op2 = crdt.insert(Some(id1), "second".to_string());
    let id2 = match &op2 {
        Operation::Insert { id, .. } => *id,
        _ => unreachable!(),
    };
    crdt.insert(Some(id2), "third".to_string());

    crdt.delete(id1);
    assert_eq!(crdt.render(), vec!["second", "third"]);
}

#[test]
fn delete_last_of_many() {
    let mut crdt = LineCRDT::new(SiteId(1));
    let op1 = crdt.insert(None, "first".to_string());
    let id1 = match &op1 {
        Operation::Insert { id, .. } => *id,
        _ => unreachable!(),
    };
    let op2 = crdt.insert(Some(id1), "second".to_string());
    let id2 = match &op2 {
        Operation::Insert { id, .. } => *id,
        _ => unreachable!(),
    };
    let op3 = crdt.insert(Some(id2), "third".to_string());
    let id3 = match &op3 {
        Operation::Insert { id, .. } => *id,
        _ => unreachable!(),
    };

    crdt.delete(id3);
    assert_eq!(crdt.render(), vec!["first", "second"]);
}

#[test]
fn delete_middle_element() {
    let mut crdt = LineCRDT::new(SiteId(1));
    let op1 = crdt.insert(None, "alpha".to_string());
    let id1 = match &op1 {
        Operation::Insert { id, .. } => *id,
        _ => unreachable!(),
    };
    let op2 = crdt.insert(Some(id1), "beta".to_string());
    let id2 = match &op2 {
        Operation::Insert { id, .. } => *id,
        _ => unreachable!(),
    };
    crdt.insert(Some(id2), "gamma".to_string());

    crdt.delete(id2);
    assert_eq!(crdt.render(), vec!["alpha", "gamma"]);
}

#[test]
fn delete_already_deleted() {
    let mut crdt = LineCRDT::new(SiteId(1));
    let op = crdt.insert(None, "ephemeral".to_string());
    let id = match &op {
        Operation::Insert { id, .. } => *id,
        _ => unreachable!(),
    };

    crdt.delete(id);
    assert!(crdt.render().is_empty());

    // Second delete should be safe (idempotent)
    crdt.delete(id);
    assert!(crdt.render().is_empty());
}

#[test]
fn insert_after_deleted_element() {
    let mut crdt = LineCRDT::new(SiteId(1));
    let op = crdt.insert(None, "deleted parent".to_string());
    let parent_id = match &op {
        Operation::Insert { id, .. } => *id,
        _ => unreachable!(),
    };

    crdt.delete(parent_id);
    assert!(crdt.render().is_empty());

    // Insert after the tombstoned element — should still work
    crdt.insert(Some(parent_id), "child of tombstone".to_string());
    assert_eq!(crdt.render(), vec!["child of tombstone"]);
}

#[test]
fn many_inserts_same_position() {
    // 10 concurrent inserts at the same position (after=None) from different sites.
    // They should be ordered deterministically by OpId (timestamp, then site).
    let mut crdt = LineCRDT::new(SiteId(99));

    // Construct 10 operations from 10 different sites, all at timestamp 1, all after=None.
    let mut ops: Vec<Operation> = Vec::new();
    for site in 0..10u64 {
        ops.push(Operation::Insert {
            id: op(1, site),
            after: None,
            content: format!("site-{}", site),
        });
    }

    // Apply in arbitrary order (reverse)
    for o in ops.into_iter().rev() {
        crdt.apply(o);
    }

    let rendered = crdt.render();
    assert_eq!(rendered.len(), 10);

    // OpId ordering: same timestamp, so ordered by site descending (higher wins position).
    // RGA: higher OpId elements are skipped over, so they come first.
    // Site 9 has highest OpId, so it should appear first.
    for i in 0..10 {
        assert_eq!(rendered[i], format!("site-{}", 9 - i));
    }
}

#[test]
fn large_document_operations() {
    let mut crdt = LineCRDT::new(SiteId(1));
    let mut last_id: Option<OpId> = None;

    for i in 0..100u64 {
        let op = crdt.insert(last_id, format!("line-{}", i));
        last_id = match &op {
            Operation::Insert { id, .. } => Some(*id),
            _ => unreachable!(),
        };
    }

    let rendered = crdt.render();
    assert_eq!(rendered.len(), 100);
    for i in 0..100 {
        assert_eq!(rendered[i], format!("line-{}", i));
    }
}

#[test]
fn interleaved_insert_delete() {
    let mut crdt = LineCRDT::new(SiteId(1));
    let mut surviving_lines: Vec<String> = Vec::new();

    let mut last_id: Option<OpId> = None;
    for i in 0..20u64 {
        let op = crdt.insert(last_id, format!("line-{}", i));
        let id = match &op {
            Operation::Insert { id, .. } => *id,
            _ => unreachable!(),
        };
        last_id = Some(id);

        // Delete every other line (odd-numbered)
        if i % 2 == 1 {
            crdt.delete(id);
        } else {
            surviving_lines.push(format!("line-{}", i));
        }
    }

    let rendered = crdt.render();
    assert_eq!(
        rendered,
        surviving_lines
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<&str>>()
    );
}

#[test]
fn render_skips_all_tombstones() {
    let mut crdt = LineCRDT::new(SiteId(1));
    let mut ids = Vec::new();

    let mut last_id: Option<OpId> = None;
    for i in 0..5u64 {
        let op = crdt.insert(last_id, format!("line-{}", i));
        let id = match &op {
            Operation::Insert { id, .. } => *id,
            _ => unreachable!(),
        };
        last_id = Some(id);
        ids.push(id);
    }

    assert_eq!(crdt.render().len(), 5);

    // Delete every element
    for id in &ids {
        crdt.delete(*id);
    }

    assert!(
        crdt.render().is_empty(),
        "All elements deleted, render should return empty vec"
    );
    assert!(
        crdt.visible_ids().is_empty(),
        "No visible ids after full deletion"
    );
}

#[test]
fn clock_advances_correctly() {
    let mut crdt = LineCRDT::new(SiteId(1));
    assert_eq!(crdt.clock(), 0, "Initial clock should be 0");

    let n = 7u64;
    let mut last_id: Option<OpId> = None;
    for _ in 0..n {
        let op = crdt.insert(last_id, "x".to_string());
        last_id = match &op {
            Operation::Insert { id, .. } => Some(*id),
            _ => unreachable!(),
        };
    }

    assert!(
        crdt.clock() >= n,
        "After {} insert operations, clock should be at least {} but was {}",
        n,
        n,
        crdt.clock()
    );

    // Deletes don't tick the clock (they reuse the target's id), but let's
    // verify the clock doesn't go backwards.
    let clock_before_delete = crdt.clock();
    if let Some(id) = last_id {
        crdt.delete(id);
    }
    assert!(
        crdt.clock() >= clock_before_delete,
        "Clock must never decrease after a delete"
    );
}
