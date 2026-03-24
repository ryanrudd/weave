use std::fs;
use weave::crdt::SiteId;
use weave::repository::{storage, CommitId, Repository};
use weave::strategy::LineCRDT;

/// Create a temp directory for test repos.
fn temp_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("weave_test_{}", rand_id()));
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Simple pseudo-random ID for temp dirs.
fn rand_id() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}

fn make_repo(site: u64) -> Repository<LineCRDT> {
    Repository::new(SiteId(site), LineCRDT::new)
}

#[test]
fn init_creates_weave_directory() {
    let dir = temp_dir();
    storage::init(&dir, SiteId(1)).unwrap();
    assert!(dir.join(".weave").exists());
    assert!(dir.join(".weave/repo.json").exists());
    assert!(dir.join(".weave/commits/0.json").exists());
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn init_fails_if_already_exists() {
    let dir = temp_dir();
    storage::init(&dir, SiteId(1)).unwrap();
    let result = storage::init(&dir, SiteId(1));
    assert!(result.is_err());
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn save_and_load_empty_repo() {
    let dir = temp_dir();
    storage::init(&dir, SiteId(42)).unwrap();
    let weave_dir = dir.join(".weave");

    let repo: Repository<LineCRDT> = storage::load(&weave_dir, LineCRDT::new).unwrap();
    assert_eq!(repo.current_branch(), "main");
    assert!(repo.tracked_files().is_empty());

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn save_load_round_trip_with_content() {
    let dir = temp_dir();
    storage::init(&dir, SiteId(1)).unwrap();
    let weave_dir = dir.join(".weave");

    // Create and save
    {
        let mut repo: Repository<LineCRDT> = storage::load(&weave_dir, LineCRDT::new).unwrap();
        {
            let doc = repo.open_file("test.txt");
            doc.append("hello".into());
            doc.append("world".into());
        }
        repo.commit("add test file");
        storage::save(&repo, &weave_dir).unwrap();
    }

    // Load and verify
    {
        let repo: Repository<LineCRDT> = storage::load(&weave_dir, LineCRDT::new).unwrap();
        assert_eq!(repo.read_file("test.txt"), Some("hello\nworld".into()));
    }

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn staging_survives_reload() {
    let dir = temp_dir();
    storage::init(&dir, SiteId(1)).unwrap();
    let weave_dir = dir.join(".weave");

    // Add content but don't commit
    {
        let mut repo: Repository<LineCRDT> = storage::load(&weave_dir, LineCRDT::new).unwrap();
        {
            let doc = repo.open_file("staged.txt");
            doc.append("not yet committed".into());
        }
        storage::save(&repo, &weave_dir).unwrap();
    }

    // Load — staged content should be present
    {
        let repo: Repository<LineCRDT> = storage::load(&weave_dir, LineCRDT::new).unwrap();
        assert_eq!(
            repo.read_file("staged.txt"),
            Some("not yet committed".into())
        );
    }

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn save_load_after_multiple_commits() {
    let dir = temp_dir();
    storage::init(&dir, SiteId(1)).unwrap();
    let weave_dir = dir.join(".weave");

    {
        let mut repo: Repository<LineCRDT> = storage::load(&weave_dir, LineCRDT::new).unwrap();
        for i in 0..5 {
            let doc = repo.open_file("log.txt");
            doc.append(format!("line {}", i));
            repo.commit(&format!("commit {}", i));
        }
        storage::save(&repo, &weave_dir).unwrap();
    }

    {
        let repo: Repository<LineCRDT> = storage::load(&weave_dir, LineCRDT::new).unwrap();
        let content = repo.read_file("log.txt").unwrap();
        for i in 0..5 {
            assert!(content.contains(&format!("line {}", i)));
        }
    }

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn save_load_preserves_branches() {
    let dir = temp_dir();
    storage::init(&dir, SiteId(1)).unwrap();
    let weave_dir = dir.join(".weave");

    {
        let mut repo: Repository<LineCRDT> = storage::load(&weave_dir, LineCRDT::new).unwrap();
        repo.create_branch("feature");
        repo.create_branch("dev");
        repo.checkout("dev").unwrap();
        storage::save(&repo, &weave_dir).unwrap();
    }

    {
        let repo: Repository<LineCRDT> = storage::load(&weave_dir, LineCRDT::new).unwrap();
        assert_eq!(repo.current_branch(), "dev");
        let branches = repo.branches();
        assert!(branches.contains_key("main"));
        assert!(branches.contains_key("feature"));
        assert!(branches.contains_key("dev"));
    }

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn save_load_after_merge() {
    let dir = temp_dir();
    storage::init(&dir, SiteId(1)).unwrap();
    let weave_dir = dir.join(".weave");

    {
        let mut repo: Repository<LineCRDT> = storage::load(&weave_dir, LineCRDT::new).unwrap();
        {
            let doc = repo.open_file("f.txt");
            doc.append("base".into());
        }
        repo.commit("base");
        repo.create_branch("feature");

        {
            let doc = repo.open_file("f.txt");
            doc.append("main line".into());
        }
        repo.commit("main edit");

        repo.checkout("feature").unwrap();
        {
            let doc = repo.open_file("f.txt");
            doc.append("feature line".into());
        }
        repo.commit("feature edit");

        repo.checkout("main").unwrap();
        repo.merge("feature").unwrap();
        storage::save(&repo, &weave_dir).unwrap();
    }

    {
        let repo: Repository<LineCRDT> = storage::load(&weave_dir, LineCRDT::new).unwrap();
        let content = repo.read_file("f.txt").unwrap();
        assert!(content.contains("base"));
        assert!(content.contains("main line"));
        assert!(content.contains("feature line"));
    }

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn find_weave_dir_from_subdirectory() {
    let dir = temp_dir();
    storage::init(&dir, SiteId(1)).unwrap();

    let subdir = dir.join("src").join("deep");
    fs::create_dir_all(&subdir).unwrap();

    let found = storage::find_weave_dir(&subdir);
    assert!(found.is_some());
    assert_eq!(found.unwrap(), dir.join(".weave"));

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn find_weave_dir_returns_none_outside_repo() {
    let dir = temp_dir();
    // Don't init — no .weave directory
    let found = storage::find_weave_dir(&dir);
    assert!(found.is_none());
    fs::remove_dir_all(&dir).ok();
}
