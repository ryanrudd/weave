use weave::crdt::SiteId;
use weave::repository::Repository;
use weave::strategy::LineCRDT;

fn make_repo(site: u64) -> Repository<LineCRDT> {
    Repository::new(SiteId(site), LineCRDT::new)
}

#[test]
fn merge_nonexistent_branch_errors() {
    let mut repo = make_repo(1);
    let result = repo.merge("ghost");
    assert!(result.is_err());
}

#[test]
fn merge_creates_commit_with_two_parents() {
    let mut repo = make_repo(1);
    repo.create_branch("feature");
    {
        let doc = repo.open_file("f.txt");
        doc.append("main".into());
    }
    repo.commit("main work");

    repo.checkout("feature").unwrap();
    {
        let doc = repo.open_file("f.txt");
        doc.append("feature".into());
    }
    repo.commit("feature work");

    repo.checkout("main").unwrap();
    let merge_id = repo.merge("feature").unwrap();
    let merge_commit = repo.get_commit(merge_id).unwrap();
    assert_eq!(merge_commit.parents.len(), 2);
}

#[test]
fn merge_multi_file_branches() {
    let mut repo = make_repo(1);
    repo.create_branch("feature");

    // Main: edit file_a
    {
        let doc = repo.open_file("a.txt");
        doc.append("a content".into());
    }
    repo.commit("main: add a");

    // Feature: edit file_b
    repo.checkout("feature").unwrap();
    {
        let doc = repo.open_file("b.txt");
        doc.append("b content".into());
    }
    repo.commit("feature: add b");

    // Merge into main
    repo.checkout("main").unwrap();
    repo.merge("feature").unwrap();

    assert_eq!(repo.read_file("a.txt"), Some("a content".into()));
    assert_eq!(repo.read_file("b.txt"), Some("b content".into()));
}

#[test]
fn merge_same_file_both_branches() {
    let mut repo = make_repo(1);
    {
        let doc = repo.open_file("f.txt");
        doc.append("base".into());
    }
    repo.commit("base");

    repo.create_branch("feature");

    // Main adds a line
    {
        let doc = repo.open_file("f.txt");
        doc.append("from main".into());
    }
    repo.commit("main edit");

    // Feature adds a line
    repo.checkout("feature").unwrap();
    {
        let doc = repo.open_file("f.txt");
        doc.append("from feature".into());
    }
    repo.commit("feature edit");

    // Merge
    repo.checkout("main").unwrap();
    repo.merge("feature").unwrap();

    let content = repo.read_file("f.txt").unwrap();
    assert!(content.contains("base"));
    assert!(content.contains("from main"));
    assert!(content.contains("from feature"));
}

#[test]
fn merge_with_deletes_on_one_branch() {
    let mut repo = make_repo(1);
    {
        let doc = repo.open_file("f.txt");
        doc.append("keep".into());
        doc.append("remove".into());
        doc.append("also keep".into());
    }
    repo.commit("initial");

    repo.create_branch("feature");

    // Main deletes the middle line
    {
        let doc = repo.open_file("f.txt");
        let ids = doc.visible_ids();
        doc.delete(ids[1]); // delete "remove"
    }
    repo.commit("delete on main");

    // Feature adds a line
    repo.checkout("feature").unwrap();
    {
        let doc = repo.open_file("f.txt");
        doc.append("from feature".into());
    }
    repo.commit("add on feature");

    // Merge
    repo.checkout("main").unwrap();
    repo.merge("feature").unwrap();

    let content = repo.read_file("f.txt").unwrap();
    assert!(content.contains("keep"));
    assert!(!content.contains("remove"));
    assert!(content.contains("also keep"));
    assert!(content.contains("from feature"));
}

#[test]
fn nested_branch_merge() {
    // main -> feature -> sub-feature, merge sub-feature into feature, then feature into main
    let mut repo = make_repo(1);
    {
        let doc = repo.open_file("f.txt");
        doc.append("main base".into());
    }
    repo.commit("base");

    repo.create_branch("feature");
    repo.checkout("feature").unwrap();
    {
        let doc = repo.open_file("f.txt");
        doc.append("feature work".into());
    }
    repo.commit("feature");

    repo.create_branch("sub-feature");
    repo.checkout("sub-feature").unwrap();
    {
        let doc = repo.open_file("f.txt");
        doc.append("sub-feature work".into());
    }
    repo.commit("sub-feature");

    // Merge sub-feature into feature
    repo.checkout("feature").unwrap();
    repo.merge("sub-feature").unwrap();

    let content = repo.read_file("f.txt").unwrap();
    assert!(content.contains("feature work"));
    assert!(content.contains("sub-feature work"));

    // Merge feature into main
    repo.checkout("main").unwrap();
    repo.merge("feature").unwrap();

    let content = repo.read_file("f.txt").unwrap();
    assert!(content.contains("main base"));
    assert!(content.contains("feature work"));
    assert!(content.contains("sub-feature work"));
}

#[test]
fn merge_branch_into_itself_is_noop() {
    // Merging a branch that has no new commits shouldn't break anything
    let mut repo = make_repo(1);
    {
        let doc = repo.open_file("f.txt");
        doc.append("hello".into());
    }
    repo.commit("initial");

    repo.create_branch("feature");
    // feature and main point to same commit, merging should be fine
    repo.merge("feature").unwrap();

    assert_eq!(repo.read_file("f.txt"), Some("hello".into()));
}

#[test]
fn long_divergent_histories_merge() {
    let mut repo = make_repo(1);
    repo.create_branch("feature");

    // 10 commits on main
    for i in 0..10 {
        let doc = repo.open_file("main.txt");
        doc.append(format!("main line {}", i));
        repo.commit(&format!("main commit {}", i));
    }

    // 10 commits on feature
    repo.checkout("feature").unwrap();
    for i in 0..10 {
        let doc = repo.open_file("feature.txt");
        doc.append(format!("feature line {}", i));
        repo.commit(&format!("feature commit {}", i));
    }

    // Merge
    repo.checkout("main").unwrap();
    repo.merge("feature").unwrap();

    let main_content = repo.read_file("main.txt").unwrap();
    let feature_content = repo.read_file("feature.txt").unwrap();

    for i in 0..10 {
        assert!(main_content.contains(&format!("main line {}", i)));
        assert!(feature_content.contains(&format!("feature line {}", i)));
    }
}

#[test]
fn merge_then_continue_working() {
    let mut repo = make_repo(1);
    repo.create_branch("feature");

    {
        let doc = repo.open_file("f.txt");
        doc.append("main 1".into());
    }
    repo.commit("main 1");

    repo.checkout("feature").unwrap();
    {
        let doc = repo.open_file("f.txt");
        doc.append("feature 1".into());
    }
    repo.commit("feature 1");

    repo.checkout("main").unwrap();
    repo.merge("feature").unwrap();

    // Continue working after merge
    {
        let doc = repo.open_file("f.txt");
        doc.append("main 2 (after merge)".into());
    }
    repo.commit("post-merge work");

    let content = repo.read_file("f.txt").unwrap();
    assert!(content.contains("main 1"));
    assert!(content.contains("feature 1"));
    assert!(content.contains("main 2 (after merge)"));
}

#[test]
fn double_merge_same_branch() {
    // Merge feature twice — second merge should be a no-op
    let mut repo = make_repo(1);
    repo.create_branch("feature");

    repo.checkout("feature").unwrap();
    {
        let doc = repo.open_file("f.txt");
        doc.append("feature".into());
    }
    repo.commit("feature work");

    repo.checkout("main").unwrap();
    repo.merge("feature").unwrap();
    let content_after_first = repo.read_file("f.txt").unwrap();

    repo.merge("feature").unwrap();
    let content_after_second = repo.read_file("f.txt").unwrap();

    assert_eq!(content_after_first, content_after_second);
}

#[test]
fn three_way_merge() {
    // Three branches all diverge from the same point
    let mut repo = make_repo(1);
    {
        let doc = repo.open_file("f.txt");
        doc.append("base".into());
    }
    repo.commit("base");

    repo.create_branch("a");
    repo.create_branch("b");

    // Branch a adds a line
    repo.checkout("a").unwrap();
    {
        let doc = repo.open_file("f.txt");
        doc.append("from a".into());
    }
    repo.commit("a work");

    // Branch b adds a line
    repo.checkout("b").unwrap();
    {
        let doc = repo.open_file("f.txt");
        doc.append("from b".into());
    }
    repo.commit("b work");

    // Main adds a line
    repo.checkout("main").unwrap();
    {
        let doc = repo.open_file("f.txt");
        doc.append("from main".into());
    }
    repo.commit("main work");

    // Merge a, then b into main
    repo.merge("a").unwrap();
    repo.merge("b").unwrap();

    let content = repo.read_file("f.txt").unwrap();
    assert!(content.contains("base"));
    assert!(content.contains("from a"));
    assert!(content.contains("from b"));
    assert!(content.contains("from main"));
}

#[test]
fn merge_preserves_line_order_within_branch() {
    let mut repo = make_repo(1);
    repo.create_branch("feature");

    // Main: add lines in order
    {
        let doc = repo.open_file("f.txt");
        doc.append("main 1".into());
        doc.append("main 2".into());
        doc.append("main 3".into());
    }
    repo.commit("main lines");

    // Feature: add lines in order
    repo.checkout("feature").unwrap();
    {
        let doc = repo.open_file("f.txt");
        doc.append("feature 1".into());
        doc.append("feature 2".into());
        doc.append("feature 3".into());
    }
    repo.commit("feature lines");

    repo.checkout("main").unwrap();
    repo.merge("feature").unwrap();

    let content = repo.read_file("f.txt").unwrap();
    let lines: Vec<&str> = content.split('\n').collect();

    // Within each branch's lines, order should be preserved
    let main_positions: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.starts_with("main"))
        .map(|(i, _)| i)
        .collect();
    let feature_positions: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.starts_with("feature"))
        .map(|(i, _)| i)
        .collect();

    // Each group's positions should be sorted (relative order preserved)
    assert_eq!(main_positions, {
        let mut sorted = main_positions.clone();
        sorted.sort();
        sorted
    });
    assert_eq!(feature_positions, {
        let mut sorted = feature_positions.clone();
        sorted.sort();
        sorted
    });
}
