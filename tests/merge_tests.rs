use weave::crdt::{MergeStrategy, SiteId};
use weave::document::Document;
use weave::strategy::LineCRDT;

/// Helper: create a document with a LineCRDT strategy for the given site.
fn make_doc(name: &str, site: u64) -> Document<LineCRDT> {
    Document::new(name.to_string(), LineCRDT::new(SiteId(site)))
}

#[test]
fn single_site_append() {
    let mut doc = make_doc("test.txt", 1);
    doc.append("hello".into());
    doc.append("world".into());

    assert_eq!(doc.lines(), vec!["hello", "world"]);
}

#[test]
fn single_site_insert_at_beginning() {
    let mut doc = make_doc("test.txt", 1);
    doc.append("second".into());
    // Insert at the beginning (after: None)
    doc.insert_after(None, "first".into());

    assert_eq!(doc.lines(), vec!["first", "second"]);
}

#[test]
fn single_site_delete() {
    let mut doc = make_doc("test.txt", 1);
    let id1 = doc.append("keep".into());
    let id2 = doc.append("remove".into());
    doc.append("keep too".into());

    doc.delete(id2);

    assert_eq!(doc.lines(), vec!["keep", "keep too"]);
}

#[test]
fn concurrent_appends_merge_deterministically() {
    // Two sites start from the same empty document and each append a line.
    // When they merge, both should converge to the same result regardless
    // of merge order. This is the core CRDT guarantee.

    // Site 1 appends "alpha"
    let mut doc1 = make_doc("test.txt", 1);
    let ops1 = {
        doc1.append("alpha".into());
        doc1.operations().to_vec()
    };

    // Site 2 appends "beta" (concurrently — doesn't know about site 1's edit)
    let mut doc2 = make_doc("test.txt", 2);
    let ops2 = {
        doc2.append("beta".into());
        doc2.operations().to_vec()
    };

    // Merge: doc1 receives doc2's ops, doc2 receives doc1's ops
    doc1.merge_remote(ops2.clone());
    doc2.merge_remote(ops1.clone());

    // Both documents must have the same content
    let result1 = doc1.lines();
    let result2 = doc2.lines();

    assert_eq!(result1, result2, "Documents diverged after merge!");
    assert_eq!(
        result1.len(),
        2,
        "Expected 2 lines after merging concurrent appends"
    );
}

#[test]
fn concurrent_inserts_at_same_position() {
    // Both sites insert a line at the beginning of an empty document.
    // The CRDT must order them deterministically.

    let mut doc1 = make_doc("test.txt", 1);
    let ops1 = {
        doc1.insert_after(None, "from site 1".into());
        doc1.operations().to_vec()
    };

    let mut doc2 = make_doc("test.txt", 2);
    let ops2 = {
        doc2.insert_after(None, "from site 2".into());
        doc2.operations().to_vec()
    };

    doc1.merge_remote(ops2.clone());
    doc2.merge_remote(ops1.clone());

    assert_eq!(
        doc1.lines(),
        doc2.lines(),
        "Concurrent inserts at same position must converge"
    );
}

#[test]
fn merge_is_idempotent() {
    // Applying the same operations twice should have no effect.
    // This matters because in a real system, ops might be delivered more than once.

    let mut doc1 = make_doc("test.txt", 1);
    doc1.append("line 1".into());
    doc1.append("line 2".into());
    let ops = doc1.operations().to_vec();

    let mut doc2 = make_doc("test.txt", 2);
    doc2.merge_remote(ops.clone());
    doc2.merge_remote(ops.clone()); // Apply again — should be a no-op

    assert_eq!(doc2.lines(), vec!["line 1", "line 2"]);
}

#[test]
fn concurrent_insert_and_delete() {
    // Site 1 has a line. Site 2 gets it, then concurrently:
    // - Site 1 inserts a new line after it
    // - Site 2 deletes the original line
    // Both should converge.

    // Shared starting state: one line
    let mut doc1 = make_doc("test.txt", 1);
    let original_id = doc1.append("original".into());
    let shared_ops = doc1.operations().to_vec();

    let mut doc2 = make_doc("test.txt", 2);
    doc2.merge_remote(shared_ops);

    // Now they diverge:
    // Site 1 inserts after "original"
    let insert_ops = {
        doc1.insert_after(Some(original_id), "inserted by site 1".into());
        // Only the new op (skip the shared "original" insert)
        doc1.operations()[1..].to_vec()
    };

    // Site 2 deletes "original"
    let delete_ops = {
        doc2.delete(original_id);
        // Only the new op
        doc2.operations()[..].to_vec()
    };

    // Merge
    doc1.merge_remote(delete_ops);
    doc2.merge_remote(insert_ops);

    let result1 = doc1.lines();
    let result2 = doc2.lines();

    assert_eq!(result1, result2, "Insert + delete must converge");
    // "original" is deleted, but "inserted by site 1" should survive
    assert!(
        result1.contains(&"inserted by site 1"),
        "Inserted line should survive even though its neighbor was deleted"
    );
    assert!(
        !result1.contains(&"original"),
        "Deleted line should not be visible"
    );
}
