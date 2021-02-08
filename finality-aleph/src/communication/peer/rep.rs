use sc_network::ReputationChange as Rep;

/// Cost scalars to be used when reporting peers.
mod cost {
    pub(crate) const MALFORMED_CATCH_UP: i32 = -1000;
    pub(crate) const PER_UNDECODABLE_BYTE: i32 = -5;
    pub(crate) const PER_SIGNATURE_CHECKED: i32 = -25;
    pub(crate) const INVALID_VIEW_CHANGE: i32 = -500;
}

pub(crate) enum PeerMisbehavior {
    UndecodablePacket(i32),
    BadSyncMessage {
        signatures_checked: i32,
        // blocks_loaded: i32,
        // equivocations_caught: i32,
    },
    MalformedSync,
    InvalidEpochId,
    // FutureMessage,
    // OutOfScopeMessage,
}

impl PeerMisbehavior {
    pub(crate) fn cost(&self) -> Rep {
        use PeerMisbehavior::*;

        match *self {
            UndecodablePacket(bytes) => Rep::new(
                bytes.saturating_mul(cost::PER_UNDECODABLE_BYTE),
                "Aleph: Bad packet",
            ),
            BadSyncMessage { signatures_checked } => Rep::new(
                cost::PER_SIGNATURE_CHECKED.saturating_mul(signatures_checked),
                "Aleph: Bad sync message",
            ),
            MalformedSync => Rep::new(cost::MALFORMED_CATCH_UP, "Aleph: Malformed sync"),
            InvalidEpochId => Rep::new(cost::INVALID_VIEW_CHANGE, "Aleph: Invalid epoch ID"),
        }
    }
}

/// Benefit scalars used to report good peers.
mod benefit {

    pub(crate) const VALIDATED_SYNC: i32 = 200;
}

pub(crate) enum PeerGoodBehavior {
    ValidatedSync,
}

impl PeerGoodBehavior {
    pub(crate) fn benefit(&self) -> Rep {
        use PeerGoodBehavior::*;

        match *self {
            ValidatedSync => Rep::new(benefit::VALIDATED_SYNC, "Aleph: Validated sync message"),
        }
    }
}
