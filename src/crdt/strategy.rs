use super::types::{OpId, Operation, SiteId};

/// The core trait that all CRDT merge strategies implement.
///
/// This is the strategy pattern — LineCRDT, CharCRDT, AstCRDT would each
/// implement this trait with different granularity, but the rest of the
/// system (Document, Repository) works with any MergeStrategy.
pub trait MergeStrategy {
    /// Apply a single operation to the local state.
    /// This must be idempotent — applying the same op twice is a no-op.
    fn apply(&mut self, op: Operation);

    /// Apply a batch of operations from a remote site.
    /// Default implementation just applies them one at a time.
    fn merge(&mut self, ops: Vec<Operation>) {
        for op in ops {
            self.apply(op);
        }
    }

    /// Generate an insert operation for new content after the given position.
    /// Returns the operation so it can be broadcast to other sites.
    fn insert(&mut self, after: Option<OpId>, content: String) -> Operation;

    /// Generate a delete operation for the element at the given id.
    /// Returns the operation so it can be broadcast to other sites.
    fn delete(&mut self, id: OpId) -> Operation;

    /// Render the current state as a vector of visible (non-deleted) strings.
    fn render(&self) -> Vec<&str>;

    /// Get the SiteId for this strategy instance.
    fn site_id(&self) -> SiteId;

    /// Get the OpId of the last visible (non-deleted) element.
    /// Used by Document to implement append.
    fn last_visible_id(&self) -> Option<OpId>;

    /// Get the current logical clock value.
    fn clock(&self) -> u64;

    /// Set the logical clock to at least the given value.
    /// Used by Repository to keep clocks consistent across branch checkouts.
    fn set_clock_minimum(&mut self, min: u64);
}
