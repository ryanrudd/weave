use super::commit::CommitId;

/// A branch is just a named pointer to a commit — same concept as git.
/// When you make a new commit on a branch, the pointer moves forward.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Branch {
    pub name: String,
    /// The commit this branch currently points to.
    pub head: CommitId,
}
