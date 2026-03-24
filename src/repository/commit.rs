use crate::crdt::Operation;

/// A unique identifier for a commit, just a simple incrementing ID for now.
/// A real system would use a content hash (like git's SHA), but this keeps
/// things simple while we prove out the merge model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct CommitId(pub u64);

/// A commit captures a set of CRDT operations applied to named files.
///
/// Unlike git (which stores snapshots), weave commits store *operations*.
/// This is the crucial difference — when merging, we replay operations
/// through the CRDT instead of diffing snapshots, which is what gives
/// us conflict-free merges.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Commit {
    pub id: CommitId,
    /// The parent commit(s). Empty for the root commit.
    /// Two parents means this is a merge commit.
    pub parents: Vec<CommitId>,
    /// The operations included in this commit, keyed by filename.
    pub operations: Vec<FileOps>,
    /// Human-readable commit message.
    pub message: String,
}

/// Operations applied to a single file in a commit.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileOps {
    pub filename: String,
    pub ops: Vec<Operation>,
}
