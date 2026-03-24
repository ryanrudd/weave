use crate::crdt::{MergeStrategy, OpId, Operation};

/// A document is a named file whose contents are managed by a CRDT strategy.
///
/// This is the main API consumers interact with. It wraps a MergeStrategy
/// and provides file-level operations like "append a line", "insert at line N",
/// and "merge changes from another document".
pub struct Document<S: MergeStrategy> {
    pub name: String,
    strategy: S,
    /// Log of all operations applied to this document, in application order.
    /// This is what gets sent to other sites when merging.
    ops_log: Vec<Operation>,
}

impl<S: MergeStrategy> Document<S> {
    pub fn new(name: String, strategy: S) -> Self {
        Document {
            name,
            strategy,
            ops_log: Vec::new(),
        }
    }

    /// Append a line at the end of the document.
    /// Returns the OpId of the inserted line (useful for inserting after it later).
    pub fn append(&mut self, content: String) -> OpId {
        let after = self.last_visible_id();
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
    pub fn merge_remote(&mut self, ops: Vec<Operation>) {
        self.strategy.merge(ops);
    }

    /// Get all operations in this document's history.
    /// Used to send our state to another site for merging.
    pub fn operations(&self) -> &[Operation] {
        &self.ops_log
    }

    /// Render the document as a single string (lines joined by newlines).
    pub fn to_string(&self) -> String {
        self.strategy.render().join("\n")
    }

    /// Render the document as a vector of lines.
    pub fn lines(&self) -> Vec<&str> {
        self.strategy.render()
    }

    /// Find the OpId of the last visible (non-deleted) line.
    /// Used internally to implement append.
    fn last_visible_id(&self) -> Option<OpId> {
        // We need access to the elements to find the last visible one.
        // For now, we track this through the ops log — the last Insert
        // that hasn't been deleted is our append target.
        //
        // This is a simplification. A production version would expose
        // this from the strategy directly.
        let deleted_ids: std::collections::HashSet<OpId> = self
            .ops_log
            .iter()
            .filter_map(|op| match op {
                Operation::Delete { id } => Some(*id),
                _ => None,
            })
            .collect();

        self.ops_log
            .iter()
            .rev()
            .filter_map(|op| match op {
                Operation::Insert { id, .. } if !deleted_ids.contains(id) => Some(*id),
                _ => None,
            })
            .next()
    }
}
