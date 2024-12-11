use frame_election_provider_support::{
    BoundedSupportsOf, ElectionProvider, ElectionProviderBase, Support,
};
use frame_support::{
    construct_runtime,
    pallet_prelude::ConstU32,
    parameter_types,
    traits::EstimateNextSessionRotation,
    weights::{RuntimeDbWeight, Weight},
};
use frame_system::pallet_prelude::BlockNumberFor;
use pallet_staking::ExposureOf;
use primitives::{
    AuthorityId, CommitteeSeats, EraValidators, SessionIndex, SessionInfoProvider,
    TotalIssuanceProvider as TotalIssuanceProviderT, DEFAULT_MAX_WINNERS,
};
use sp_core::{bounded_vec, ConstU64, H256};
use sp_runtime::{
    impl_opaque_keys,
    testing::TestXt,
    traits::{ConvertInto, IdentityLookup},
    BoundedVec,
};
use sp_staking::{EraIndex, Exposure};

use super::*;
use crate as pallet_committee_management;

type Block = frame_system::mocking::MockBlock<TestRuntime>;
pub(crate) type AccountId = u64;
pub(crate) type Balance = u128;

construct_runtime!(
    pub enum TestRuntime
    {
        System: frame_system,
        Balances: pallet_balances,
        Staking: pallet_staking,
        History: pallet_session::historical,
        Session: pallet_session,
        Aleph: pallet_aleph,
        CommitteeManagement: pallet_committee_management,
        Timestamp: pallet_timestamp,
        Elections: pallet_elections,
    }
);

impl_opaque_keys! {
    pub struct TestRuntimeSessionKeys {
        pub aleph: pallet_aleph::Pallet<TestRuntime>,
    }
}

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub BlockWeights: frame_system::limits::BlockWeights =
        frame_system::limits::BlockWeights::simple_max(Weight::from_parts(1024, 0));
    pub const TestRuntimeDbWeight: RuntimeDbWeight = RuntimeDbWeight {
        read: 25,
        write: 100
    };
}

impl frame_system::Config for TestRuntime {
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
    type DbWeight = TestRuntimeDbWeight;
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

impl pallet_balances::Config for TestRuntime {
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

pub struct MockElectionProvider;
impl ElectionProviderBase for MockElectionProvider {
    type AccountId = AccountId;
    type BlockNumber = BlockNumberFor<TestRuntime>;
    type Error = ();
    type DataProvider = Staking;
    type MaxWinners = MaxWinners;
}

fn self_support(v: AccountId) -> Support<AccountId> {
    Support {
        total: 2137,
        voters: vec![(v, 2137)],
    }
}

impl ElectionProvider for MockElectionProvider {
    fn ongoing() -> bool {
        false
    }

    fn elect() -> Result<BoundedSupportsOf<Self>, Self::Error> {
        let elected_validators = ElectedValidators::get();
        Ok(elected_validators
            .into_iter()
            .map(|v| (v.clone(), self_support(v)))
            .collect::<Vec<_>>()
            .try_into()
            .unwrap())
    }
}

pub struct ZeroEraPayout;
impl pallet_staking::EraPayout<u128> for ZeroEraPayout {
    fn era_payout(_: u128, _: u128, _: u64) -> (u128, u128) {
        (0, 0)
    }
}

parameter_types! {
    pub const SessionsPerEra: SessionIndex = 3;
    pub static BondingDuration: u32 = 3;
}

impl pallet_staking::Config for TestRuntime {
    type Currency = Balances;
    type CurrencyBalance = u128;
    type UnixTime = pallet_timestamp::Pallet<Self>;
    type CurrencyToVote = ();
    type RewardRemainder = ();
    type RuntimeEvent = RuntimeEvent;
    type Slash = ();
    type Reward = ();
    type SessionsPerEra = SessionsPerEra;
    type SlashDeferDuration = ();
    type AdminOrigin = frame_system::EnsureRoot<Self::AccountId>;
    type BondingDuration = BondingDuration;
    type SessionInterface = Self;
    type EraPayout = ZeroEraPayout;
    type NextNewSession = Session;
    type MaxExposurePageSize = ConstU32<64>;
    type OffendingValidatorsThreshold = ();
    type ElectionProvider = MockElectionProvider;
    type GenesisElectionProvider = Self::ElectionProvider;
    type VoterList = pallet_staking::UseNominatorsAndValidatorsMap<TestRuntime>;
    type TargetList = pallet_staking::UseValidatorsMap<Self>;
    type NominationsQuota = pallet_staking::FixedNominationsQuota<16>;
    type MaxUnlockingChunks = ConstU32<32>;
    type MaxControllersInDeprecationBatch = ConstU32<64>;
    type HistoryDepth = ConstU32<84>;
    type EventListeners = ();
    type BenchmarkingConfig = pallet_staking::TestBenchmarkingConfig;
    type WeightInfo = ();
}

impl pallet_session::historical::Config for TestRuntime {
    type FullIdentification = Exposure<AccountId, Balance>;
    type FullIdentificationOf = ExposureOf<TestRuntime>;
}

pub struct SessionInfoImpl;
impl SessionInfoProvider<BlockNumberFor<TestRuntime>> for SessionInfoImpl {
    fn current_session() -> SessionIndex {
        pallet_session::CurrentIndex::<TestRuntime>::get()
    }
    fn next_session_block_number(
        current_block: BlockNumberFor<TestRuntime>,
    ) -> Option<BlockNumberFor<TestRuntime>> {
        <TestRuntime as pallet_session::Config>::NextSessionRotation::estimate_next_session_rotation(
            current_block,
        )
        .0
    }
}

parameter_types! {
    pub const SessionPeriod: u32 = 5;
    pub const Offset: u64 = 0;
}

impl pallet_session::Config for TestRuntime {
    type RuntimeEvent = RuntimeEvent;
    type ValidatorId = u64;
    type ValidatorIdOf = ConvertInto;
    type ShouldEndSession = pallet_session::PeriodicSessions<SessionPeriod, Offset>;
    type NextSessionRotation = pallet_session::PeriodicSessions<SessionPeriod, Offset>;
    type SessionManager = Aleph;
    type SessionHandler = (Aleph,);
    type Keys = TestRuntimeSessionKeys;
    type WeightInfo = ();
}

parameter_types! {
    pub static NewElectedValidators: BoundedVec<AccountId, MaxWinners> = bounded_vec![];
    pub static ElectedValidators: BoundedVec<AccountId, MaxWinners> = bounded_vec![];
    pub static ActiveEra: EraIndex = 0;
    pub static CurrentEra: EraIndex = 0;
    pub static EraCommitteSeats: CommitteeSeats = CommitteeSeats::default();
    pub static NextEraCommitteSeats: CommitteeSeats = CommitteeSeats::default();
    pub static ReservedValidators: Vec<AccountId> = vec![];
    pub static NonReservedValidators: Vec<AccountId> = vec![];
    pub static NextReservedValidators: Vec<AccountId> = vec![];
    pub static NextNonReservedValidators: Vec<AccountId> = vec![];
    pub static CurrentEraValidators: EraValidators<AccountId> = EraValidators::default();
    pub static MaxWinners: u32 = DEFAULT_MAX_WINNERS;
}

impl pallet_aleph::Config for TestRuntime {
    type AuthorityId = AuthorityId;
    type RuntimeEvent = RuntimeEvent;
    type SessionInfoProvider = SessionInfoImpl;
    type SessionManager = SessionAndEraManager<
        Staking,
        Elections,
        pallet_session::historical::NoteHistoricalRoot<TestRuntime, Staking>,
        TestRuntime,
    >;
    type NextSessionAuthorityProvider = Session;
    type TotalIssuanceProvider = TotalIssuanceProvider;
}

impl<C> frame_system::offchain::SendTransactionTypes<C> for TestRuntime
where
    RuntimeCall: From<C>,
{
    type Extrinsic = TestXt<RuntimeCall, ()>;
    type OverarchingCall = RuntimeCall;
}

parameter_types! {
    pub const MinimumPeriod: u64 = 3;
}

impl pallet_timestamp::Config for TestRuntime {
    type Moment = u64;
    type OnTimestampSet = ();
    type MinimumPeriod = ConstU64<5>;
    type WeightInfo = ();
}

pub struct TotalIssuanceProvider;
impl TotalIssuanceProviderT for TotalIssuanceProvider {
    fn get() -> Balance {
        pallet_balances::Pallet::<TestRuntime>::total_issuance()
    }
}

impl pallet_elections::Config for TestRuntime {
    type RuntimeEvent = RuntimeEvent;
    type DataProvider = Staking;
    type ValidatorProvider = Staking;
    type MaxWinners = MaxWinners;
    type BannedValidators = CommitteeManagement;
}

impl Config for TestRuntime {
    type RuntimeEvent = RuntimeEvent;
    type BanHandler = Elections;
    type EraInfoProvider = Staking;
    type ValidatorProvider = Elections;
    type ValidatorRewardsHandler = Staking;
    type ValidatorExtractor = Staking;
    type FinalityCommitteeManager = Aleph;
    type SessionPeriod = SessionPeriod;
    type AbftScoresProvider = Aleph;
}
