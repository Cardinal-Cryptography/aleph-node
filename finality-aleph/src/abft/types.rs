//! Types common for current & legacy abft used across finality-aleph

pub use primitives::{NodeCount, NodeIndex};

/// A recipient of a message, either a specific node or everyone.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Recipient {
    Everyone,
    Node(NodeIndex),
}

impl From<legacy_aleph_bft::Recipient> for Recipient {
    fn from(recipient: legacy_aleph_bft::Recipient) -> Self {
        match recipient {
            legacy_aleph_bft::Recipient::Everyone => Recipient::Everyone,
            legacy_aleph_bft::Recipient::Node(id) => Recipient::Node(id.into()),
        }
    }
}

impl From<current_aleph_bft::Recipient> for Recipient {
    fn from(recipient: current_aleph_bft::Recipient) -> Self {
        match recipient {
            current_aleph_bft::Recipient::Everyone => Recipient::Everyone,
            current_aleph_bft::Recipient::Node(id) => Recipient::Node(id.into()),
        }
    }
}
