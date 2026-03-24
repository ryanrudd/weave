/// Identifies a unique site (user/replica) in the distributed system.
/// Each collaborator gets a unique SiteId so we can tell edits apart.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct SiteId(pub u64);

/// A Lamport timestamp — a logical clock that increments with each operation.
/// Combined with SiteId, this gives every operation a globally unique, orderable identity.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct LamportTimestamp(pub u64);

/// Uniquely identifies any operation across all sites.
/// The combination of (timestamp, site) is guaranteed unique because:
/// - Two ops from the same site have different timestamps (local clock increments)
/// - Two ops from different sites have different site IDs
///
/// Ordering: timestamp first, then site ID as tiebreaker — this is what makes
/// concurrent inserts at the same position resolve deterministically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct OpId {
    pub timestamp: LamportTimestamp,
    pub site: SiteId,
}

impl Ord for OpId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.timestamp
            .cmp(&other.timestamp)
            .then(self.site.cmp(&other.site))
    }
}

impl PartialOrd for OpId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// The kinds of changes that can be made to a document.
/// These are the operations that get replicated between sites.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Operation {
    /// Insert content after the element with the given OpId.
    /// `after: None` means insert at the very beginning.
    Insert {
        id: OpId,
        after: Option<OpId>,
        content: String,
    },
    /// Mark an element as deleted (tombstone — we keep it around so
    /// concurrent operations that reference it still make sense).
    Delete { id: OpId },
}
