use crate::crdt::{MergeStrategy, OpId, Operation};

/// A document is a named file whose contents are managed by a CRDT strategy.
///
/// This is the main API consumers interact with. It wraps a MergeStrategy
/// and provides file-level operations like "append a line", "insert at line N",
/// and "merge changes from another document".
pub struct Document<S: MergeStrategy> {
    pub name: String,
    strategy: S,
    /// Log of locally-generated operations (insert, delete calls on this doc).
    /// Does NOT include ops applied via merge_remote.
    ops_log: Vec<Operation>,
    /// How many ops from ops_log have already been included in a commit.
    /// This lets us return only uncommitted ops when it's time to commit.
    committed_offset: usize,
}

impl<S: MergeStrategy> Document<S> {
    pub fn new(name: String, strategy: S) -> Self {
        Document {
            name,
            strategy,
            ops_log: Vec::new(),
            committed_offset: 0,
        }
    }

    /// Append a line at the end of the document.
    /// Returns the OpId of the inserted line (useful for inserting after it later).
    pub fn append(&mut self, content: String) -> OpId {
        let after = self.strategy.last_visible_id();
        let op = self.strategy.insert(after, content);
        let id = match &op {
            Operation::Insert { id, .. } => *id,
            _ => unreachable!(),
        };
        self.ops_log.push(op);
        id
    }

    /// Insert a line after the element with the given OpId.
    pub fn insert_after(&mut self, after: Option<OpId>, content: String) -> OpId {
        let op = self.strategy.insert(after, content);
        let id = match &op {
            Operation::Insert { id, .. } => *id,
            _ => unreachable!(),
        };
        self.ops_log.push(op);
        id
    }

    /// Delete the line with the given OpId.
    pub fn delete(&mut self, id: OpId) {
        let op = self.strategy.delete(id);
        self.ops_log.push(op);
    }

    /// Merge operations from a remote document into this one.
    /// This is the conflict-free part — applying remote ops will always succeed.
    /// These ops are NOT added to the local ops_log (they're someone else's ops).
    pub fn merge_remote(&mut self, ops: Vec<Operation>) {
        self.strategy.merge(ops);
    }

    /// Apply a previously-generated local operation and record it in the ops log.
    /// Used when restoring staged operations from disk.
    pub fn apply_local(&mut self, op: Operation) {
        self.strategy.apply(op.clone());
        self.ops_log.push(op);
    }

    /// Get all operations in this document's history.
    /// Used for low-level access (e.g., tests that simulate two standalone docs).
    pub fn operations(&self) -> &[Operation] {
        &self.ops_log
    }

    /// Get only the operations that haven't been committed yet.
    /// This is what Repository::commit() should use.
    pub fn uncommitted_operations(&self) -> &[Operation] {
        &self.ops_log[self.committed_offset..]
    }

    /// Mark all current operations as committed.
    /// Called by Repository after a successful commit.
    pub fn mark_committed(&mut self) {
        self.committed_offset = self.ops_log.len();
    }

    /// Render the document as a vector of lines.
    pub fn lines(&self) -> Vec<&str> {
        self.strategy.render()
    }

    /// Get the current logical clock value from the underlying strategy.
    pub fn clock(&self) -> u64 {
        self.strategy.clock()
    }

    /// Get the OpIds of all visible (non-deleted) elements in order.
    /// Used to identify lines for deletion during re-add.
    pub fn visible_ids(&self) -> Vec<OpId> {
        self.strategy.visible_ids()
    }
}

impl<S: MergeStrategy> std::fmt::Display for Document<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.strategy.render().join("\n"))
    }
}
