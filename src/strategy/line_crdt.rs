use crate::crdt::{LamportTimestamp, MergeStrategy, OpId, Operation, SiteId};

/// A single element in the RGA sequence.
/// Each line in the document becomes one of these.
#[derive(Debug, Clone)]
struct Element {
    /// Unique identity of this element.
    id: OpId,
    /// The element this one was inserted after. None = head of document.
    after: Option<OpId>,
    /// The text content of this line.
    content: String,
    /// Tombstone flag. When true, this element has been "deleted" but we
    /// keep it around so concurrent operations that reference it still work.
    deleted: bool,
}

/// Line-level CRDT using the RGA (Replicated Growable Array) algorithm.
///
/// The document is a sequence of Elements. Each element knows its ID and
/// what it was inserted after. To find where an element goes in the sequence:
/// 1. Find the element it was inserted after
/// 2. Walk right, skipping any elements with a higher OpId (they "win" the position)
/// 3. Insert before the first element with a lower OpId
///
/// This gives deterministic ordering for concurrent inserts at the same position.
pub struct LineCRDT {
    site: SiteId,
    clock: LamportTimestamp,
    /// The sequence of all elements (including tombstones).
    elements: Vec<Element>,
}

impl LineCRDT {
    pub fn new(site: SiteId) -> Self {
        LineCRDT {
            site,
            clock: LamportTimestamp(0),
            elements: Vec::new(),
        }
    }

    /// Advance our logical clock, ensuring it's ahead of any timestamp we've seen.
    fn tick(&mut self) -> LamportTimestamp {
        self.clock.0 += 1;
        self.clock
    }

    /// Update our clock to be at least as recent as a remote timestamp.
    /// This is how Lamport clocks stay in sync — when you receive a remote
    /// op, you bump your clock to max(yours, theirs) + 1.
    fn observe(&mut self, remote_ts: LamportTimestamp) {
        if remote_ts.0 >= self.clock.0 {
            self.clock.0 = remote_ts.0;
        }
    }

    /// Find the index of an element by its OpId.
    fn find_index(&self, id: &OpId) -> Option<usize> {
        self.elements.iter().position(|e| e.id == *id)
    }

    /// Find where to insert a new element that goes after `after_id`.
    /// Returns the index where the new element should be placed.
    ///
    /// RGA rule: after finding the target, scan right past any elements
    /// with higher OpIds (they were concurrent and "won" the tiebreak).
    /// Insert before the first element with a lower OpId.
    fn find_insert_position(&self, after_id: Option<OpId>, new_id: OpId) -> usize {
        // Start position: right after the "after" element, or at 0 if inserting at head
        let start = match after_id {
            Some(ref id) => self.find_index(id).map(|i| i + 1).unwrap_or(0),
            None => 0,
        };

        // Scan right: skip elements that have a higher OpId than ours.
        // These are concurrent inserts that "win" the position over us.
        let mut pos = start;
        while pos < self.elements.len() {
            let existing = &self.elements[pos];
            // Only compare against elements that share the same "after" parent.
            // Once we hit an element with a different parent, we've gone too far.
            if existing.after != after_id {
                break;
            }
            // If the existing element has a higher ID, it goes before us — skip it.
            if existing.id > new_id {
                pos += 1;
            } else {
                break;
            }
        }

        pos
    }
}

impl MergeStrategy for LineCRDT {
    fn apply(&mut self, op: Operation) {
        match op {
            Operation::Insert { id, after, content } => {
                // Idempotency: if we already have this op, skip it
                if self.find_index(&id).is_some() {
                    return;
                }

                // Update our clock to stay in sync
                self.observe(id.timestamp);

                let pos = self.find_insert_position(after, id);
                self.elements.insert(
                    pos,
                    Element {
                        id,
                        after,
                        content,
                        deleted: false,
                    },
                );
            }
            Operation::Delete { id } => {
                // Find the element and mark it as a tombstone
                if let Some(idx) = self.find_index(&id) {
                    self.elements[idx].deleted = true;
                }
            }
        }
    }

    fn insert(&mut self, after: Option<OpId>, content: String) -> Operation {
        let ts = self.tick();
        let id = OpId {
            timestamp: ts,
            site: self.site,
        };

        let op = Operation::Insert {
            id,
            after,
            content,
        };

        // Apply locally
        self.apply(op.clone());
        op
    }

    fn delete(&mut self, id: OpId) -> Operation {
        let op = Operation::Delete { id };
        self.apply(op.clone());
        op
    }

    fn render(&self) -> Vec<&str> {
        self.elements
            .iter()
            .filter(|e| !e.deleted)
            .map(|e| e.content.as_str())
            .collect()
    }

    fn site_id(&self) -> SiteId {
        self.site
    }

    fn last_visible_id(&self) -> Option<OpId> {
        self.elements
            .iter()
            .rev()
            .find(|e| !e.deleted)
            .map(|e| e.id)
    }

    fn clock(&self) -> u64 {
        self.clock.0
    }

    fn set_clock_minimum(&mut self, min: u64) {
        if min > self.clock.0 {
            self.clock.0 = min;
        }
    }
}
