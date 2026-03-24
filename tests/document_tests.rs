use weave::crdt::SiteId;
use weave::document::Document;
use weave::strategy::LineCRDT;

/// Helper: create a document with a LineCRDT strategy for the given site.
fn make_doc(name: &str, site: u64) -> Document<LineCRDT> {
    Document::new(name.to_string(), LineCRDT::new(SiteId(site)))
}

#[test]
fn new_document_is_empty() {
    let doc = make_doc("test.txt", 1);
    assert!(doc.lines().is_empty());
    assert!(doc.operations().is_empty());
}

#[test]
fn append_single_line() {
    let mut doc = make_doc("test.txt", 1);
    doc.append("hello".into());
    assert_eq!(doc.lines(), vec!["hello"]);
}

#[test]
fn append_multiple_lines() {
    let mut doc = make_doc("test.txt", 1);
    doc.append("first".into());
    doc.append("second".into());
    doc.append("third".into());
    assert_eq!(doc.lines(), vec!["first", "second", "third"]);
}

#[test]
fn insert_after_specific_line() {
    let mut doc = make_doc("test.txt", 1);
    let id1 = doc.append("line A".into());
    doc.append("line C".into());
    doc.insert_after(Some(id1), "line B".into());
    assert_eq!(doc.lines(), vec!["line A", "line B", "line C"]);
}

#[test]
fn insert_at_beginning() {
    let mut doc = make_doc("test.txt", 1);
    doc.append("second".into());
    doc.append("third".into());
    doc.insert_after(None, "first".into());
    assert_eq!(doc.lines(), vec!["first", "second", "third"]);
}

#[test]
fn delete_removes_from_render() {
    let mut doc = make_doc("test.txt", 1);
    doc.append("keep".into());
    let id = doc.append("remove me".into());
    doc.append("also keep".into());
    doc.delete(id);
    assert_eq!(doc.lines(), vec!["keep", "also keep"]);
}

#[test]
fn delete_preserves_other_lines() {
    let mut doc = make_doc("test.txt", 1);
    let id1 = doc.append("alpha".into());
    let id2 = doc.append("beta".into());
    let id3 = doc.append("gamma".into());
    doc.delete(id2);

    let lines = doc.lines();
    assert_eq!(lines.len(), 2);
    assert!(lines.contains(&"alpha"));
    assert!(lines.contains(&"gamma"));
    assert!(!lines.contains(&"beta"));

    // Verify the remaining visible ids still correspond to id1 and id3
    let visible = doc.visible_ids();
    assert!(visible.contains(&id1));
    assert!(visible.contains(&id3));
    assert!(!visible.contains(&id2));
}

#[test]
fn to_string_joins_with_newlines() {
    let mut doc = make_doc("test.txt", 1);
    doc.append("line one".into());
    doc.append("line two".into());
    doc.append("line three".into());
    assert_eq!(doc.to_string(), "line one\nline two\nline three");
}

#[test]
fn operations_tracks_local_ops() {
    let mut doc = make_doc("test.txt", 1);
    assert_eq!(doc.operations().len(), 0);

    doc.append("a".into());
    assert_eq!(doc.operations().len(), 1);

    doc.append("b".into());
    assert_eq!(doc.operations().len(), 2);

    let id = doc.append("c".into());
    doc.delete(id);
    // 3 inserts + 1 delete = 4 operations
    assert_eq!(doc.operations().len(), 4);
}

#[test]
fn uncommitted_operations_after_mark() {
    let mut doc = make_doc("test.txt", 1);
    doc.append("first".into());
    doc.append("second".into());
    assert_eq!(doc.uncommitted_operations().len(), 2);

    doc.mark_committed();
    assert!(doc.uncommitted_operations().is_empty());

    // New operations after commit are uncommitted
    doc.append("third".into());
    assert_eq!(doc.uncommitted_operations().len(), 1);
}

#[test]
fn merge_remote_applies_external_ops() {
    let mut doc1 = make_doc("test.txt", 1);
    doc1.append("from site 1".into());
    let ops = doc1.operations().to_vec();

    let mut doc2 = make_doc("test.txt", 2);
    doc2.append("from site 2".into());
    doc2.merge_remote(ops);

    let lines = doc2.lines();
    assert_eq!(lines.len(), 2);
    assert!(lines.contains(&"from site 1"));
    assert!(lines.contains(&"from site 2"));

    // merge_remote should NOT add to local operations log
    assert_eq!(doc2.operations().len(), 1);
}

#[test]
fn visible_ids_matches_render_count() {
    let mut doc = make_doc("test.txt", 1);
    doc.append("one".into());
    doc.append("two".into());
    let id3 = doc.append("three".into());
    doc.append("four".into());

    assert_eq!(doc.visible_ids().len(), doc.lines().len());
    assert_eq!(doc.visible_ids().len(), 4);

    doc.delete(id3);
    assert_eq!(doc.visible_ids().len(), doc.lines().len());
    assert_eq!(doc.visible_ids().len(), 3);
}
