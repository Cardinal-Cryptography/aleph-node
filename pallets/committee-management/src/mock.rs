use frame_election_provider_support::{
    data_provider, DataProviderBounds, ElectionDataProvider, VoteWeight,
};
use frame_support::{
    construct_runtime,
    pallet_prelude::ConstU32,
    parameter_types,
    traits::EstimateNextSessionRotation,
    weights::{RuntimeDbWeight, Weight},
};
use frame_system::pallet_prelude::BlockNumberFor;
use pallet_elections::ValidatorProvider;
use pallet_session::SessionManager;
use primitives::{
    AuthorityId, BannedValidators, CommitteeSeats, EraValidators, SessionIndex,
    SessionInfoProvider, TotalIssuanceProvider as TotalIssuanceProviderT,
    ValidatorProvider as EraValidatorProvider, DEFAULT_MAX_WINNERS,
};
use sp_core::H256;
use sp_runtime::{
    impl_opaque_keys,
    testing::TestXt,
    traits::{ConvertInto, IdentityLookup},
    BoundedVec,
};
use sp_staking::EraIndex;

use super::*;
use crate as pallet_committee_management;

const SESSIONS_PER_ERA: SessionIndex = 3;

type Block = frame_system::mocking::MockBlock<Test>;
pub(crate) type AccountId = u64;
pub(crate) type Balance = u128;

construct_runtime!(
    pub enum Test
    {
        System: frame_system,
        Balances: pallet_balances,
        Session: pallet_session,
        Aleph: pallet_aleph,
        Elections: pallet_elections,
        CommitteeManagement: pallet_committee_management,
    }
);

impl_opaque_keys! {
    pub struct TestSessionKeys {
        pub aleph: pallet_aleph::Pallet<Test>,
    }
}

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub BlockWeights: frame_system::limits::BlockWeights =
        frame_system::limits::BlockWeights::simple_max(Weight::from_parts(1024, 0));
    pub const TestDbWeight: RuntimeDbWeight = RuntimeDbWeight {
        read: 25,
        write: 100
    };
}

// TODO use test derive
impl frame_system::Config for Test {
    type BaseCallFilter = frame_support::traits::Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type RuntimeTask = RuntimeTask;
    type Nonce = u64;
    type Block = Block;
    type Hash = H256;
    type Hashing = sp_runtime::traits::BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type RuntimeEvent = RuntimeEvent;
    type BlockHashCount = BlockHashCount;
    type DbWeight = TestDbWeight;
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = pallet_balances::AccountData<u128>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ();
    type OnSetCode = ();
    type MaxConsumers = frame_support::traits::ConstU32<16>;
}

parameter_types! {
    pub const ExistentialDeposit: u128 = 1;
}

// TODO use test derive
impl pallet_balances::Config for Test {
    type Balance = u128;
    type MaxReserves = ();
    type ReserveIdentifier = [u8; 8];
    type DustRemoval = ();
    type RuntimeEvent = RuntimeEvent;
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
    type MaxLocks = ();
    type FreezeIdentifier = ();
    type MaxHolds = ConstU32<0>;
    type MaxFreezes = ConstU32<0>;
    type RuntimeHoldReason = ();
    type RuntimeFreezeReason = RuntimeFreezeReason;
}

pub struct SessionInfoImpl;
impl SessionInfoProvider<BlockNumberFor<Test>> for SessionInfoImpl {
    fn current_session() -> SessionIndex {
        pallet_session::CurrentIndex::<Test>::get()
    }
    fn next_session_block_number(
        current_block: BlockNumberFor<Test>,
    ) -> Option<BlockNumberFor<Test>> {
        <Test as pallet_session::Config>::NextSessionRotation::estimate_next_session_rotation(
            current_block,
        )
        .0
    }
}

parameter_types! {
    pub const SessionPeriod: u32 = 5;
    pub const Offset: u64 = 0;
}

impl pallet_session::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type ValidatorId = u64;
    type ValidatorIdOf = ConvertInto;
    type ShouldEndSession = pallet_session::PeriodicSessions<SessionPeriod, Offset>;
    type NextSessionRotation = pallet_session::PeriodicSessions<SessionPeriod, Offset>;
    type SessionManager = Aleph;
    type SessionHandler = (Aleph,);
    type Keys = TestSessionKeys;
    type WeightInfo = ();
}

pub struct MockEraInfoProvider;
impl EraInfoProvider for MockEraInfoProvider {
    type AccountId = AccountId;

    fn active_era() -> Option<EraIndex> {
        Some(ActiveEra::get())
    }

    fn current_era() -> Option<EraIndex> {
        Some(CurrentEra::get())
    }

    fn sessions_per_era() -> SessionIndex {
        SESSIONS_PER_ERA
    }

    fn elected_validators(_era: sp_staking::EraIndex) -> Vec<Self::AccountId> {
        ElectedValidators::get()
    }

    fn era_start_session_index(_era: sp_staking::EraIndex) -> Option<SessionIndex> {
        // TODO implement
        None
    }
}

parameter_types! {
    pub static Validators: Vec<u64> = vec![1, 2, 3];
    pub static NextValidators: Vec<u64> = vec![1, 2, 3];
    // pub static Authorities: Vec<UintAuthorityId> =
    //     vec![UintAuthorityId(1), UintAuthorityId(2), UintAuthorityId(3)];
    pub static ForceSessionEnd: bool = false;
    pub static SessionLength: u64 = 2;
    pub static SessionChanged: bool = false;
    pub static TestSessionChanged: bool = false;
    pub static Disabled: bool = false;
    // Stores if `on_before_session_end` was called
    pub static BeforeSessionEndCalled: bool = false;
}

pub struct MockSessionManager;
impl SessionManager<AccountId> for MockSessionManager {
    fn end_session(_: SessionIndex) {}
    fn start_session(_: SessionIndex) {}
    fn new_session(_: SessionIndex) -> Option<Vec<u64>> {
        if !TestSessionChanged::get() {
            Validators::mutate(|v| {
                *v = NextValidators::get().clone();
                Some(v.clone())
            })
        } else if Disabled::mutate(|l| std::mem::replace(&mut *l, false)) {
            // If there was a disabled validator, underlying conditions have changed
            // so we return `Some`.
            Some(Validators::get().clone())
        } else {
            None
        }
    }
}

impl pallet_aleph::Config for Test {
    type AuthorityId = AuthorityId;
    type RuntimeEvent = RuntimeEvent;
    type SessionInfoProvider = SessionInfoImpl;
    type SessionManager =
        SessionAndEraManager<MockEraInfoProvider, Elections, MockSessionManager, Test>;
    type NextSessionAuthorityProvider = Session;
    type TotalIssuanceProvider = TotalIssuanceProvider;
}

pub struct MockValidatorProvider;

parameter_types! {
    pub static ActiveEra: EraIndex = 0;
    pub static CurrentEra: EraIndex = 0;
    pub static ElectedValidators: Vec<u64> = vec![1, 2, 3];
}
impl ValidatorProvider for MockValidatorProvider {
    type AccountId = AccountId;

    fn elected_validators(era: EraIndex) -> Vec<Self::AccountId> {
        ElectedValidators::get()
    }
}

impl BannedValidators for MockValidatorProvider {
    type AccountId = AccountId;

    fn banned() -> Vec<Self::AccountId> {
        vec![]
    }
}

type MaxVotesPerVoter = ConstU32<1>;
type AccountIdBoundedVec = BoundedVec<AccountId, MaxVotesPerVoter>;
type Vote = (AccountId, VoteWeight, AccountIdBoundedVec);

pub struct MockDataProvider;
impl ElectionDataProvider for MockDataProvider {
    type AccountId = AccountId;
    type BlockNumber = u64;
    type MaxVotesPerVoter = MaxVotesPerVoter;

    fn electable_targets(
        _maybe_max_len: DataProviderBounds,
    ) -> data_provider::Result<Vec<AccountId>> {
        // TODO implement
        Ok(vec![])
    }

    fn electing_voters(_maybe_max_len: DataProviderBounds) -> data_provider::Result<Vec<Vote>> {
        Ok(vec![])
    }

    fn desired_targets() -> data_provider::Result<u32> {
        Ok(0)
    }

    fn next_election_prediction(_now: u64) -> u64 {
        0
    }
}

impl pallet_elections::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type DataProvider = MockDataProvider;
    type ValidatorProvider = MockValidatorProvider;
    type MaxWinners = ConstU32<DEFAULT_MAX_WINNERS>;
    type BannedValidators = MockValidatorProvider;
}

impl<C> frame_system::offchain::SendTransactionTypes<C> for Test
where
    RuntimeCall: From<C>,
{
    type Extrinsic = TestXt<RuntimeCall, ()>;
    type OverarchingCall = RuntimeCall;
}

pub struct TotalIssuanceProvider;
impl TotalIssuanceProviderT for TotalIssuanceProvider {
    fn get() -> Balance {
        pallet_balances::Pallet::<Test>::total_issuance()
    }
}

parameter_types! {
    pub static EraCommitteSeats: CommitteeSeats = CommitteeSeats::default();
    pub static ReservedValidators: Vec<AccountId> = vec![];
    pub static NonReservedValidators: Vec<AccountId> = vec![];
}

pub struct MockEraValidatorProvider;
impl EraValidatorProvider for MockEraValidatorProvider {
    type AccountId = AccountId;
    fn current_era_validators() -> EraValidators<Self::AccountId> {
        EraValidators { reserved: ReservedValidators::get(), non_reserved: NonReservedValidators::get() }
    }
    fn current_era_committee_size() -> CommitteeSeats {
        EraCommitteSeats::get()
    }
}

pub struct MockExtractor;
impl ValidatorExtractor for MockExtractor {
    type AccountId = AccountId;
    fn remove_validator(_who: &Self::AccountId) {}
}

pub struct MockRewardsHandler;
impl ValidatorRewardsHandler for MockRewardsHandler {
    type AccountId = AccountId;
    fn add_rewards(_rewards: impl IntoIterator<Item = (Self::AccountId, u32)>) {}

    fn validator_totals(_era: EraIndex) -> Vec<(Self::AccountId, u128)> {
        vec![]
    }
}

impl Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type BanHandler = Elections;
    type EraInfoProvider = MockEraInfoProvider;
    type ValidatorProvider = MockEraValidatorProvider;
    type ValidatorRewardsHandler = MockRewardsHandler;
    type ValidatorExtractor = MockExtractor;
    type FinalityCommitteeManager = Aleph;
    type SessionPeriod = SessionPeriod;
    type AbftScoresProvider = Aleph;
}
