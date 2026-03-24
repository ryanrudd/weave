use weave::crdt::SiteId;
use weave::repository::Repository;
use weave::strategy::LineCRDT;

/// Helper: create a repo with LineCRDT strategy.
fn make_repo(site: u64) -> Repository<LineCRDT> {
    Repository::new(SiteId(site), LineCRDT::new)
}

#[test]
fn basic_commit_and_read() {
    let mut repo = make_repo(1);

    {
        let doc = repo.open_file("hello.txt");
        doc.append("hello world".into());
    }

    let commit_id = repo.commit("Add hello.txt");

    let commit = repo.get_commit(commit_id).unwrap();
    assert_eq!(commit.message, "Add hello.txt");
    assert_eq!(commit.parents.len(), 1); // parent is the root commit

    assert_eq!(repo.read_file("hello.txt"), Some("hello world".into()));
}

#[test]
fn branch_and_checkout() {
    let mut repo = make_repo(1);

    // Add a file on main
    {
        let doc = repo.open_file("file.txt");
        doc.append("line on main".into());
    }
    repo.commit("Add file on main");

    // Create a feature branch and switch to it
    repo.create_branch("feature");
    repo.checkout("feature").unwrap();
    assert_eq!(repo.current_branch(), "feature");

    // File should still be visible (inherited from main)
    assert_eq!(repo.read_file("file.txt"), Some("line on main".into()));
}

#[test]
fn checkout_nonexistent_branch_fails() {
    let mut repo = make_repo(1);
    let result = repo.checkout("nope");
    assert!(result.is_err());
}

#[test]
fn divergent_branches_merge_cleanly() {
    let mut repo = make_repo(1);

    // Shared base: one file with one line
    {
        let doc = repo.open_file("shared.txt");
        doc.append("base line".into());
    }
    repo.commit("Initial content");

    // Create feature branch (points at same commit as main)
    repo.create_branch("feature");

    // Add a line on main
    {
        let doc = repo.open_file("shared.txt");
        doc.append("added on main".into());
    }
    repo.commit("Edit on main");

    // Switch to feature, add a different line
    repo.checkout("feature").unwrap();
    {
        let doc = repo.open_file("shared.txt");
        doc.append("added on feature".into());
    }
    repo.commit("Edit on feature");

    // Switch back to main and merge feature
    repo.checkout("main").unwrap();
    let merge_id = repo.merge("feature").unwrap();

    // Verify merge commit has two parents
    let merge_commit = repo.get_commit(merge_id).unwrap();
    assert_eq!(merge_commit.parents.len(), 2);

    // Verify all three lines are present
    let content = repo.read_file("shared.txt").unwrap();
    assert!(content.contains("base line"), "Base line missing");
    assert!(content.contains("added on main"), "Main's edit missing");
    assert!(content.contains("added on feature"), "Feature's edit missing");
}

#[test]
fn merge_independent_files() {
    let mut repo = make_repo(1);

    repo.create_branch("feature");

    // Main edits file_a
    {
        let doc = repo.open_file("file_a.txt");
        doc.append("content a".into());
    }
    repo.commit("Add file_a on main");

    // Feature edits file_b
    repo.checkout("feature").unwrap();
    {
        let doc = repo.open_file("file_b.txt");
        doc.append("content b".into());
    }
    repo.commit("Add file_b on feature");

    // Merge feature into main — both files should exist
    repo.checkout("main").unwrap();
    repo.merge("feature").unwrap();

    assert_eq!(repo.read_file("file_a.txt"), Some("content a".into()));
    assert_eq!(repo.read_file("file_b.txt"), Some("content b".into()));
}

#[test]
fn multiple_commits_then_merge() {
    let mut repo = make_repo(1);

    repo.create_branch("feature");

    // Several commits on main
    {
        let doc = repo.open_file("log.txt");
        doc.append("main commit 1".into());
    }
    repo.commit("Main commit 1");
    {
        let doc = repo.open_file("log.txt");
        doc.append("main commit 2".into());
    }
    repo.commit("Main commit 2");

    // Several commits on feature
    repo.checkout("feature").unwrap();
    {
        let doc = repo.open_file("log.txt");
        doc.append("feature commit 1".into());
    }
    repo.commit("Feature commit 1");
    {
        let doc = repo.open_file("log.txt");
        doc.append("feature commit 2".into());
    }
    repo.commit("Feature commit 2");

    // Merge
    repo.checkout("main").unwrap();
    repo.merge("feature").unwrap();

    let content = repo.read_file("log.txt").unwrap();
    assert!(content.contains("main commit 1"));
    assert!(content.contains("main commit 2"));
    assert!(content.contains("feature commit 1"));
    assert!(content.contains("feature commit 2"));
}
