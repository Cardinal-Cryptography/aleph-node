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
    AuthorityId, BanHandler, BannedValidators, CommitteeSeats, EraManager, EraValidators, SessionIndex, SessionInfoProvider, TotalIssuanceProvider as TotalIssuanceProviderT, ValidatorProvider as EraValidatorProvider,
};
use sp_core::H256;
use sp_runtime::{
    impl_opaque_keys,
    testing::TestXt,
    traits::{ConvertInto, IdentityLookup},
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

parameter_types! {
    // pub static Validators: Vec<u64> = vec![1, 2, 3];
    // pub static NextValidators: Vec<u64> = vec![1, 2, 3];
    // pub static Authorities: Vec<UintAuthorityId> =
    //     vec![UintAuthorityId(1), UintAuthorityId(2), UintAuthorityId(3)];
    pub static ForceSessionEnd: bool = false;
    pub static SessionChanged: bool = false;
    pub static TestSessionChanged: bool = false;
    pub static Disabled: bool = false;
    // Stores if `on_before_session_end` was called
    pub static BeforeSessionEndCalled: bool = false;
    pub static NewElectedValidators: Vec<AccountId> = vec![];
    pub static ElectedValidators: Vec<AccountId> = vec![];
    pub static ActiveEra: EraIndex = 0;
    pub static CurrentEra: EraIndex = 0;
    pub static EraCommitteSeats: CommitteeSeats = CommitteeSeats::default();
    pub static NextEraCommitteSeats: CommitteeSeats = CommitteeSeats::default();
    pub static ReservedValidators: Vec<AccountId> = vec![];
    pub static NonReservedValidators: Vec<AccountId> = vec![];
    pub static NextReservedValidators: Vec<AccountId> = vec![];
    pub static NextNonReservedValidators: Vec<AccountId> = vec![];
    pub static CurrentEraValidators: EraValidators<AccountId> = EraValidators::default();
}

// Periodic Era Manager, every era lasts Self::sessions_per_era() and
// starts on session with index equal a multiple of Self::sessions_per_era()
pub struct MockEraSessionManager;
impl SessionManager<AccountId> for MockEraSessionManager {
	/// Plan a new session potentially trigger a new era.
    fn new_session(session_index: SessionIndex) -> Option<Vec<AccountId>> {
        // TODO add support for forcing new eras;
        if session_index % Self::sessions_per_era() != 0 {
            return None
        }

        let new_elected_validators = NewElectedValidators::get();
        ElectedValidators::set(new_elected_validators.clone());

        // trigger new era

		CurrentEra::mutate(|ce| { *ce += 1; });

        Some(new_elected_validators)
    }

	/// Start a session potentially starting an era.
    fn start_session(start_session: SessionIndex) {
		let next_active_era = Self::active_era().unwrap() + 1;
        // check if next era is planned, i.e. current_era index got increamented and we are in the last session of of active_era
        if next_active_era == Self::current_era().unwrap() {
            if start_session == next_active_era * Self::sessions_per_era() {
                ActiveEra::mutate(|ae| { *ae += 1;});
            }
        }
    }

    fn end_session(_: SessionIndex) {}
}

impl EraInfoProvider for MockEraSessionManager{
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

    fn elected_validators(_era: EraIndex) -> Vec<Self::AccountId> {
        ElectedValidators::get()
    }

    fn era_start_session_index(era: EraIndex) -> Option<SessionIndex> {
       Some(era * Self::sessions_per_era())
    }
}

impl EraManager for MockEraSessionManager {
    fn on_new_era(_era: EraIndex) {
        let elected_committee = ElectedValidators::get();

        let retain_elected = |vals: Vec<AccountId>| -> Vec<AccountId> {
            vals
                .into_iter()
                .filter(|v| elected_committee.contains(v))
                .collect()
        };
        let reserved_validators = NextReservedValidators::get();
        let non_reserved_validators = NextNonReservedValidators::get();
        let committee_size = NextEraCommitteSeats::get();

        CurrentEraValidators::set(EraValidators {
            reserved: retain_elected(reserved_validators),
            non_reserved: retain_elected(non_reserved_validators),
        });
        EraCommitteSeats::set(committee_size);
    }
}

impl pallet_aleph::Config for Test {
    type AuthorityId = AuthorityId;
    type RuntimeEvent = RuntimeEvent;
    type SessionInfoProvider = SessionInfoImpl;
    type SessionManager =
        SessionAndEraManager<MockEraSessionManager, MockEraSessionManager, MockEraSessionManager, Test>;
    type NextSessionAuthorityProvider = Session;
    type TotalIssuanceProvider = TotalIssuanceProvider;
}

pub struct MockValidatorProvider;
impl ValidatorProvider for MockValidatorProvider {
    type AccountId = AccountId;

    fn elected_validators(_era: EraIndex) -> Vec<Self::AccountId> {
        ElectedValidators::get()
    }
}

impl BannedValidators for MockValidatorProvider {
    type AccountId = AccountId;

    fn banned() -> Vec<Self::AccountId> {
        vec![]
    }
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

pub struct MockBanHandler;
impl BanHandler for MockBanHandler{
    type AccountId = AccountId;
    fn can_ban(_who: &Self::AccountId) -> bool {
        false
    }
}

impl Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type BanHandler = MockBanHandler;
    type EraInfoProvider = MockEraSessionManager;
    type ValidatorProvider = MockEraValidatorProvider;
    type ValidatorRewardsHandler = MockRewardsHandler;
    type ValidatorExtractor = MockExtractor;
    type FinalityCommitteeManager = Aleph;
    type SessionPeriod = SessionPeriod;
    type AbftScoresProvider = Aleph;
}
