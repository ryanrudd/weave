use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::crdt::{MergeStrategy, SiteId};

use super::branch::Branch;
use super::commit::{Commit, CommitId};
use super::repo::Repository;

/// On-disk metadata for a repository (everything except commits).
#[derive(serde::Serialize, serde::Deserialize)]
struct RepoMeta {
    site: SiteId,
    current_branch: String,
    branches: HashMap<String, Branch>,
    next_commit_id: u64,
    global_clock: u64,
}

/// Find the .weave directory by walking up from the given path.
pub fn find_weave_dir(from: &Path) -> Option<PathBuf> {
    let mut current = from.to_path_buf();
    loop {
        let candidate = current.join(".weave");
        if candidate.is_dir() {
            return Some(candidate);
        }
        if !current.pop() {
            return None;
        }
    }
}

/// Initialize a new .weave directory at the given path.
pub fn init(path: &Path, site: SiteId) -> Result<(), String> {
    let weave_dir = path.join(".weave");
    if weave_dir.exists() {
        return Err("Already a weave repository".to_string());
    }

    let commits_dir = weave_dir.join("commits");
    fs::create_dir_all(&commits_dir).map_err(|e| e.to_string())?;

    // Create initial empty commit
    let root_commit = Commit {
        id: CommitId(0),
        parents: vec![],
        operations: vec![],
        message: "Initial commit".to_string(),
    };
    let commit_json = serde_json::to_string_pretty(&root_commit).map_err(|e| e.to_string())?;
    fs::write(commits_dir.join("0.json"), commit_json).map_err(|e| e.to_string())?;

    // Create repo metadata
    let mut branches = HashMap::new();
    branches.insert(
        "main".to_string(),
        Branch {
            name: "main".to_string(),
            head: CommitId(0),
        },
    );

    let meta = RepoMeta {
        site,
        current_branch: "main".to_string(),
        branches,
        next_commit_id: 1,
        global_clock: 0,
    };
    let meta_json = serde_json::to_string_pretty(&meta).map_err(|e| e.to_string())?;
    fs::write(weave_dir.join("repo.json"), meta_json).map_err(|e| e.to_string())?;

    Ok(())
}

/// Save a repository's state to its .weave directory.
pub fn save<S: MergeStrategy>(repo: &Repository<S>, weave_dir: &Path) -> Result<(), String> {
    let commits_dir = weave_dir.join("commits");
    fs::create_dir_all(&commits_dir).map_err(|e| e.to_string())?;

    // Save all commits
    for (id, commit) in repo.commits() {
        let path = commits_dir.join(format!("{}.json", id.0));
        let json = serde_json::to_string_pretty(commit).map_err(|e| e.to_string())?;
        fs::write(path, json).map_err(|e| e.to_string())?;
    }

    // Save metadata
    let meta = RepoMeta {
        site: repo.site,
        current_branch: repo.current_branch().to_string(),
        branches: repo.branches().clone(),
        next_commit_id: repo.next_commit_id(),
        global_clock: repo.global_clock(),
    };
    let meta_json = serde_json::to_string_pretty(&meta).map_err(|e| e.to_string())?;
    fs::write(weave_dir.join("repo.json"), meta_json).map_err(|e| e.to_string())?;

    Ok(())
}

/// Load a repository from a .weave directory.
pub fn load<S: MergeStrategy>(weave_dir: &Path, strategy_factory: fn(SiteId) -> S) -> Result<Repository<S>, String> {
    let meta_json = fs::read_to_string(weave_dir.join("repo.json")).map_err(|e| e.to_string())?;
    let meta: RepoMeta = serde_json::from_str(&meta_json).map_err(|e| e.to_string())?;

    // Load all commits
    let commits_dir = weave_dir.join("commits");
    let mut commits = HashMap::new();
    for entry in fs::read_dir(&commits_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().map(|e| e == "json").unwrap_or(false) {
            let json = fs::read_to_string(&path).map_err(|e| e.to_string())?;
            let commit: Commit = serde_json::from_str(&json).map_err(|e| e.to_string())?;
            commits.insert(commit.id, commit);
        }
    }

    Ok(Repository::from_parts(
        meta.site,
        commits,
        meta.branches,
        meta.current_branch,
        meta.next_commit_id,
        meta.global_clock,
        strategy_factory,
    ))
}
