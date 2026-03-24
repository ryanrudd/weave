use std::collections::HashMap;

use crate::crdt::{MergeStrategy, Operation, SiteId};
use crate::document::Document;

use super::branch::Branch;
use super::commit::{Commit, CommitId, FileOps};

/// The top-level repository. Manages branches, commit history, and
/// the working state of all tracked documents.
///
/// Generic over the merge strategy, so the same repo logic works
/// with line-level, char-level, or AST-level CRDTs.
pub struct Repository<S: MergeStrategy> {
    pub site: SiteId,
    /// All commits, indexed by ID.
    commits: HashMap<CommitId, Commit>,
    /// All branches, indexed by name.
    branches: HashMap<String, Branch>,
    /// The currently checked-out branch name.
    current_branch: String,
    /// Working state: documents being edited before the next commit.
    /// Each document tracks its own CRDT state and uncommitted ops.
    working_docs: HashMap<String, Document<S>>,
    /// Counter for generating commit IDs.
    next_commit_id: u64,
    /// Global logical clock — ensures OpIds are unique across branch checkouts.
    /// Each CRDT instance gets this as a minimum when created.
    global_clock: u64,
    /// Factory function to create new strategy instances.
    /// We need this because when we check out a branch or create a new file,
    /// we need to build up a fresh CRDT and replay operations into it.
    strategy_factory: fn(SiteId) -> S,
}

impl<S: MergeStrategy> Repository<S> {
    /// Create a new repository with an initial empty commit on "main".
    pub fn new(site: SiteId, strategy_factory: fn(SiteId) -> S) -> Self {
        let root_commit = Commit {
            id: CommitId(0),
            parents: vec![],
            operations: vec![],
            message: "Initial commit".to_string(),
        };

        let main_branch = Branch {
            name: "main".to_string(),
            head: CommitId(0),
        };

        let mut commits = HashMap::new();
        commits.insert(CommitId(0), root_commit);

        let mut branches = HashMap::new();
        branches.insert("main".to_string(), main_branch);

        Repository {
            site,
            commits,
            branches,
            current_branch: "main".to_string(),
            working_docs: HashMap::new(),
            next_commit_id: 1,
            global_clock: 0,
            strategy_factory,
        }
    }

    /// Get or create a working document by filename.
    /// If the file doesn't exist yet, creates a new empty document with the
    /// global clock set so that new operations get unique IDs.
    pub fn open_file(&mut self, filename: &str) -> &mut Document<S> {
        let site = self.site;
        let factory = self.strategy_factory;
        let clock = self.global_clock;
        self.working_docs
            .entry(filename.to_string())
            .or_insert_with(|| {
                let mut strategy = factory(site);
                strategy.set_clock_minimum(clock);
                Document::new(filename.to_string(), strategy)
            })
    }

    /// Commit all pending operations across all working documents.
    /// Returns the new commit's ID.
    pub fn commit(&mut self, message: &str) -> CommitId {
        let parent = self.branches[&self.current_branch].head;
        let id = CommitId(self.next_commit_id);
        self.next_commit_id += 1;

        // Collect only uncommitted ops from each document
        let operations: Vec<FileOps> = self
            .working_docs
            .iter()
            .filter_map(|(filename, doc)| {
                let ops = doc.uncommitted_operations().to_vec();
                if ops.is_empty() {
                    None
                } else {
                    Some(FileOps {
                        filename: filename.clone(),
                        ops,
                    })
                }
            })
            .collect();

        let commit = Commit {
            id,
            parents: vec![parent],
            operations,
            message: message.to_string(),
        };

        self.commits.insert(id, commit);
        self.branches.get_mut(&self.current_branch).unwrap().head = id;

        // Update global clock to the max across all working docs, and mark committed
        for doc in self.working_docs.values_mut() {
            let doc_clock = doc.clock();
            if doc_clock > self.global_clock {
                self.global_clock = doc_clock;
            }
            doc.mark_committed();
        }

        id
    }

    /// Create a new branch pointing at the same commit as the current branch.
    pub fn create_branch(&mut self, name: &str) {
        let head = self.branches[&self.current_branch].head;
        self.branches.insert(
            name.to_string(),
            Branch {
                name: name.to_string(),
                head,
            },
        );
    }

    /// Switch to a different branch, rebuilding working state by replaying
    /// all operations from the root up to that branch's head.
    pub fn checkout(&mut self, branch_name: &str) -> Result<(), String> {
        if !self.branches.contains_key(branch_name) {
            return Err(format!("Branch '{}' does not exist", branch_name));
        }

        self.current_branch = branch_name.to_string();
        self.rebuild_working_state();
        Ok(())
    }

    /// Merge another branch into the current branch.
    /// This is where the CRDT magic happens — we find operations that the
    /// current branch hasn't seen and replay them through the CRDT.
    pub fn merge(&mut self, source_branch: &str) -> Result<CommitId, String> {
        if !self.branches.contains_key(source_branch) {
            return Err(format!("Branch '{}' does not exist", source_branch));
        }

        let current_head = self.branches[&self.current_branch].head;
        let source_head = self.branches[source_branch].head;

        // Find the common ancestor
        let ancestor = self.find_common_ancestor(current_head, source_head);

        // Collect all ops from source branch that happened after the ancestor
        let source_ops = self.collect_ops_since(source_head, ancestor);

        // Apply source ops to our working documents — the CRDT handles ordering
        for file_ops in &source_ops {
            let doc = self.open_file(&file_ops.filename);
            doc.merge_remote(file_ops.ops.clone());
        }

        // Create a merge commit with two parents
        let id = CommitId(self.next_commit_id);
        self.next_commit_id += 1;

        let commit = Commit {
            id,
            parents: vec![current_head, source_head],
            operations: source_ops,
            message: format!("Merge '{}' into '{}'", source_branch, self.current_branch),
        };

        self.commits.insert(id, commit);
        self.branches.get_mut(&self.current_branch).unwrap().head = id;

        Ok(id)
    }

    /// Get the current branch name.
    pub fn current_branch(&self) -> &str {
        &self.current_branch
    }

    /// Get a commit by ID.
    pub fn get_commit(&self, id: CommitId) -> Option<&Commit> {
        self.commits.get(&id)
    }

    /// Render a file's current contents from the working state.
    pub fn read_file(&self, filename: &str) -> Option<String> {
        self.working_docs.get(filename).map(|doc| doc.to_string())
    }

    /// Get uncommitted operations for each working document.
    /// Used by the storage layer to persist staged changes.
    pub fn uncommitted_ops(&self) -> Vec<(&String, &[Operation])> {
        self.working_docs
            .iter()
            .map(|(name, doc)| (name, doc.uncommitted_operations()))
            .collect()
    }

    /// List all tracked file names.
    pub fn tracked_files(&self) -> Vec<&str> {
        let mut files: Vec<&str> = self.working_docs.keys().map(|s| s.as_str()).collect();
        files.sort();
        files
    }

    /// Rebuild working documents by replaying all operations from root
    /// to the current branch's head.
    fn rebuild_working_state(&mut self) {
        self.working_docs.clear();

        let head = self.branches[&self.current_branch].head;
        let commit_chain = self.ancestor_chain(head);

        // Collect all operations first, then apply them.
        // We separate reading from self.commits and writing to self.working_docs
        // to satisfy the borrow checker — can't borrow self mutably and immutably
        // at the same time.
        let all_ops: Vec<(String, Vec<Operation>)> = commit_chain
            .iter()
            .rev()
            .filter_map(|id| self.commits.get(id))
            .flat_map(|commit| &commit.operations)
            .map(|file_ops| (file_ops.filename.clone(), file_ops.ops.clone()))
            .collect();

        for (filename, ops) in all_ops {
            let doc = self.open_file(&filename);
            doc.merge_remote(ops);
        }

        // Update global clock from rebuilt docs so new ops get unique IDs
        for doc in self.working_docs.values() {
            let doc_clock = doc.clock();
            if doc_clock > self.global_clock {
                self.global_clock = doc_clock;
            }
        }
    }

    /// Walk from a commit back to root, collecting the chain of commit IDs.
    fn ancestor_chain(&self, from: CommitId) -> Vec<CommitId> {
        let mut chain = vec![];
        let mut stack = vec![from];
        let mut visited = std::collections::HashSet::new();

        // BFS to handle merge commits (multiple parents)
        while let Some(id) = stack.pop() {
            if visited.contains(&id) {
                continue;
            }
            visited.insert(id);
            chain.push(id);

            if let Some(commit) = self.commits.get(&id) {
                for parent in &commit.parents {
                    stack.push(*parent);
                }
            }
        }

        chain
    }

    /// Find the common ancestor of two commits by walking both histories.
    fn find_common_ancestor(&self, a: CommitId, b: CommitId) -> CommitId {
        let a_ancestors: std::collections::HashSet<CommitId> =
            self.ancestor_chain(a).into_iter().collect();

        // Walk b's history until we find something in a's history
        let b_chain = self.ancestor_chain(b);
        for id in b_chain {
            if a_ancestors.contains(&id) {
                return id;
            }
        }

        // Fallback to root
        CommitId(0)
    }

    /// Collect all file operations from commits between `from` and `ancestor` (exclusive).
    fn collect_ops_since(&self, from: CommitId, ancestor: CommitId) -> Vec<FileOps> {
        let chain = self.ancestor_chain(from);
        let mut all_ops: HashMap<String, Vec<Operation>> = HashMap::new();

        // Walk from newest to ancestor, collecting ops
        for commit_id in &chain {
            if *commit_id == ancestor {
                break;
            }
            if let Some(commit) = self.commits.get(commit_id) {
                for file_ops in &commit.operations {
                    all_ops
                        .entry(file_ops.filename.clone())
                        .or_default()
                        .extend(file_ops.ops.clone());
                }
            }
        }

        all_ops
            .into_iter()
            .map(|(filename, ops)| FileOps { filename, ops })
            .collect()
    }

    // --- Accessors for storage layer ---

    /// Iterate over all commits.
    pub fn commits(&self) -> &HashMap<CommitId, Commit> {
        &self.commits
    }

    /// Get all branches.
    pub fn branches(&self) -> &HashMap<String, Branch> {
        &self.branches
    }

    /// Get the next commit ID counter.
    pub fn next_commit_id(&self) -> u64 {
        self.next_commit_id
    }

    /// Get the global clock value.
    pub fn global_clock(&self) -> u64 {
        self.global_clock
    }

    /// Reconstruct a Repository from its persisted parts.
    /// Used by the storage layer when loading from disk.
    pub fn from_parts(
        site: SiteId,
        commits: HashMap<CommitId, Commit>,
        branches: HashMap<String, Branch>,
        current_branch: String,
        next_commit_id: u64,
        global_clock: u64,
        strategy_factory: fn(SiteId) -> S,
    ) -> Self {
        let mut repo = Repository {
            site,
            commits,
            branches,
            current_branch: current_branch.clone(),
            working_docs: HashMap::new(),
            next_commit_id,
            global_clock,
            strategy_factory,
        };
        // Rebuild working state from commit history
        repo.rebuild_working_state();
        repo
    }
}
