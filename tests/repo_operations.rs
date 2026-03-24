use weave::crdt::SiteId;
use weave::repository::{CommitId, Repository};
use weave::strategy::LineCRDT;

fn make_repo(site: u64) -> Repository<LineCRDT> {
    Repository::new(SiteId(site), LineCRDT::new)
}

#[test]
fn new_repo_has_initial_commit() {
    let repo = make_repo(1);
    let commit = repo.get_commit(CommitId(0)).unwrap();
    assert_eq!(commit.message, "Initial commit");
    assert!(commit.parents.is_empty());
    assert!(commit.operations.is_empty());
}

#[test]
fn new_repo_starts_on_main() {
    let repo = make_repo(1);
    assert_eq!(repo.current_branch(), "main");
}

#[test]
fn commit_returns_incrementing_ids() {
    let mut repo = make_repo(1);
    {
        let doc = repo.open_file("a.txt");
        doc.append("line".into());
    }
    let id1 = repo.commit("first");
    {
        let doc = repo.open_file("a.txt");
        doc.append("another".into());
    }
    let id2 = repo.commit("second");
    assert_eq!(id1, CommitId(1));
    assert_eq!(id2, CommitId(2));
}

#[test]
fn commit_with_no_changes_creates_empty_commit() {
    let mut repo = make_repo(1);
    let id = repo.commit("empty commit");
    let commit = repo.get_commit(id).unwrap();
    assert!(commit.operations.is_empty());
}

#[test]
fn multi_file_commit() {
    let mut repo = make_repo(1);
    {
        let doc = repo.open_file("a.txt");
        doc.append("file a".into());
    }
    {
        let doc = repo.open_file("b.txt");
        doc.append("file b".into());
    }
    let id = repo.commit("two files");
    let commit = repo.get_commit(id).unwrap();
    assert_eq!(commit.operations.len(), 2);
}

#[test]
fn tracked_files_sorted() {
    let mut repo = make_repo(1);
    repo.open_file("z.txt");
    repo.open_file("a.txt");
    repo.open_file("m.txt");
    let files = repo.tracked_files();
    assert_eq!(files, vec!["a.txt", "m.txt", "z.txt"]);
}

#[test]
fn read_file_returns_none_for_unknown() {
    let repo = make_repo(1);
    assert!(repo.read_file("nope.txt").is_none());
}

#[test]
fn commit_parent_chain() {
    let mut repo = make_repo(1);
    {
        let doc = repo.open_file("f.txt");
        doc.append("a".into());
    }
    let c1 = repo.commit("first");
    {
        let doc = repo.open_file("f.txt");
        doc.append("b".into());
    }
    let c2 = repo.commit("second");
    {
        let doc = repo.open_file("f.txt");
        doc.append("c".into());
    }
    let c3 = repo.commit("third");

    assert_eq!(repo.get_commit(c1).unwrap().parents, vec![CommitId(0)]);
    assert_eq!(repo.get_commit(c2).unwrap().parents, vec![c1]);
    assert_eq!(repo.get_commit(c3).unwrap().parents, vec![c2]);
}

#[test]
fn branch_from_non_main() {
    let mut repo = make_repo(1);
    {
        let doc = repo.open_file("f.txt");
        doc.append("main".into());
    }
    repo.commit("on main");

    repo.create_branch("dev");
    repo.checkout("dev").unwrap();
    {
        let doc = repo.open_file("f.txt");
        doc.append("dev".into());
    }
    repo.commit("on dev");

    // Branch from dev (not main)
    repo.create_branch("feature");
    repo.checkout("feature").unwrap();

    // Feature should see both main and dev content
    let content = repo.read_file("f.txt").unwrap();
    assert!(content.contains("main"));
    assert!(content.contains("dev"));
}

#[test]
fn checkout_preserves_committed_state() {
    let mut repo = make_repo(1);
    {
        let doc = repo.open_file("f.txt");
        doc.append("committed".into());
    }
    repo.commit("save it");

    // Checkout same branch (should rebuild)
    repo.checkout("main").unwrap();
    assert_eq!(repo.read_file("f.txt"), Some("committed".into()));
}

#[test]
fn multiple_branches_independent() {
    let mut repo = make_repo(1);
    repo.create_branch("a");
    repo.create_branch("b");

    repo.checkout("a").unwrap();
    {
        let doc = repo.open_file("a.txt");
        doc.append("from branch a".into());
    }
    repo.commit("branch a work");

    repo.checkout("b").unwrap();
    // Branch b should NOT see a.txt (it was created after branching)
    assert!(repo.read_file("a.txt").is_none());
}

#[test]
fn open_file_creates_if_not_exists() {
    let mut repo = make_repo(1);
    {
        let doc = repo.open_file("new.txt");
        doc.append("hello".into());
    }
    assert_eq!(repo.read_file("new.txt"), Some("hello".into()));
}

#[test]
fn commit_only_captures_uncommitted_ops() {
    let mut repo = make_repo(1);

    // First commit: one line
    {
        let doc = repo.open_file("f.txt");
        doc.append("line 1".into());
    }
    let c1 = repo.commit("first");

    // Second commit: one more line
    {
        let doc = repo.open_file("f.txt");
        doc.append("line 2".into());
    }
    let c2 = repo.commit("second");

    // First commit should have 1 op, second should have 1 op
    let commit1 = repo.get_commit(c1).unwrap();
    let commit2 = repo.get_commit(c2).unwrap();
    let ops1: usize = commit1.operations.iter().map(|fo| fo.ops.len()).sum();
    let ops2: usize = commit2.operations.iter().map(|fo| fo.ops.len()).sum();
    assert_eq!(ops1, 1);
    assert_eq!(ops2, 1);
}
