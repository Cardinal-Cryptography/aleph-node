use sc_network::ReputationChange as Rep;

/// Cost scalars to be used when reporting peers.
mod cost {
    pub(crate) const PER_UNDECODABLE_BYTE: i32 = -5;
    pub(crate) const UNKNOWN_VOTER: i32 = -150;
    pub(crate) const BAD_SIGNATURE: i32 = -100;
    pub(crate) const OUT_OF_SCOPE_RESPONSE: i32 = -500;
}

pub(crate) enum PeerMisbehavior {
    UndecodablePacket(i32),
    UnknownVoter,
    BadSignature,
    OutOfScopeResponse,
}

impl PeerMisbehavior {
    pub(crate) fn cost(&self) -> Rep {
        use PeerMisbehavior::*;

        match *self {
            UndecodablePacket(bytes) => Rep::new(
                bytes.saturating_mul(cost::PER_UNDECODABLE_BYTE),
                "Aleph: Bad packet",
            ),
            UnknownVoter => Rep::new(cost::UNKNOWN_VOTER, "Aleph: Unknown voter"),
            BadSignature => Rep::new(cost::BAD_SIGNATURE, "Aleph: Bad signature"),
            OutOfScopeResponse => Rep::new(
                cost::OUT_OF_SCOPE_RESPONSE,
                "Aleph: Out-of-scope response message",
            ),
        }
    }
}

/// Benefit scalars used to report good peers.
mod benefit {
    // NOTE: Not sure if we actually want to give rep for a simple fetch request.
    pub(crate) const GOOD_FETCH_REQUEST: i32 = 0;
    pub(crate) const GOOD_FETCH_RESPONSE: i32 = 100;
    pub(crate) const GOOD_MULTICAST: i32 = 100;
}

pub(crate) enum PeerGoodBehavior {
    FetchRequest,
    FetchResponse,
    Multicast,
}

impl PeerGoodBehavior {
    pub(crate) fn benefit(&self) -> Rep {
        use PeerGoodBehavior::*;

        match *self {
            FetchRequest => Rep::new(benefit::GOOD_FETCH_REQUEST, "Aleph: Good fetch request"),
            FetchResponse => Rep::new(benefit::GOOD_FETCH_RESPONSE, "Aleph: Good fetch response"),
            Multicast => Rep::new(benefit::GOOD_MULTICAST, "Aleph: Good multicast message"),
        }
    }
}
