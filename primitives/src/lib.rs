#![allow(clippy::too_many_arguments, clippy::unnecessary_mut_passed)]
#![cfg_attr(not(feature = "std"), no_std)]
use codec::{Decode, Encode};
use sp_core::crypto::KeyTypeId;
use sp_runtime::ConsensusEngineId;
use sp_std::vec::Vec;

pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"alp0");

// Same as GRANDPA_ENGINE_ID because as of right now substrate sends only
// grandpa justifications over the network.
// TODO: change this once https://github.com/paritytech/substrate/issues/8172 will be resolved.
pub const ALEPH_ENGINE_ID: ConsensusEngineId = *b"FRNK";

mod app {
    use sp_application_crypto::{app_crypto, ed25519};
    app_crypto!(ed25519, crate::KEY_TYPE);
}

sp_application_crypto::with_pair! {
    pub type AuthorityPair = app::Pair;
}
pub type AuthoritySignature = app::Signature;
pub type AuthorityId = app::Public;

pub use sp_staking::SessionIndex;
pub const DEFAULT_SESSIONS_PER_ERA: SessionIndex = 4 * 24;
pub const DEFAULT_SESSION_PERIOD: u32 = 900;
pub const DEFAULT_MILLISECS_PER_BLOCK: u64 = 1000;

pub const TOKEN_DECIMALS: u32 = 12;
pub const ADDRESSES_ENCODING: u32 = 42;
pub const DEFAULT_UNIT_CREATION_DELAY: u64 = 300;

pub type Balance = u128;

#[derive(Encode, Decode, PartialEq, Eq, sp_std::fmt::Debug)]
pub enum ApiError {
    DecodeKey,
}

sp_api::decl_runtime_apis! {
    pub trait AlephSessionApi
    {
        fn next_session_authorities() -> Result<Vec<AuthorityId>, ApiError>;
        fn authorities() -> Vec<AuthorityId>;
        fn session_period() -> u32;
        fn millisecs_per_block() -> u64;
    }
}

pub mod staking {
    use super::{Balance, TOKEN_DECIMALS};
    use sp_runtime::Perbill;

    pub fn era_payout(miliseconds_per_era: u64) -> (Balance, Balance) {
        const YEARLY_INFLATION: Balance = 30_000_000 * 10u128.pow(TOKEN_DECIMALS);
        // Milliseconds per year for the Julian year (365.25 days).
        const MILLISECONDS_PER_YEAR: u64 = 1000 * 3600 * 24 * 36525 / 100;

        let portion = Perbill::from_rational(miliseconds_per_era, MILLISECONDS_PER_YEAR);
        let total_payout = portion * YEARLY_INFLATION;
        let validators_payout = Perbill::from_percent(90) * total_payout;
        let rest = total_payout - validators_payout;

        (validators_payout, rest)
    }
}
