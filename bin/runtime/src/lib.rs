#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit = "256"]

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

use pallet_staking::EraIndex;
use sp_api::impl_runtime_apis;
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_core::{crypto::KeyTypeId, OpaqueMetadata};
use sp_runtime::{
    create_runtime_str, generic, impl_opaque_keys,
    traits::{
        AccountIdLookup, BlakeTwo256, Block as BlockT, ConvertInto, IdentifyAccount, OpaqueKeys,
        Verify,
    },
    transaction_validity::{TransactionSource, TransactionValidity},
    ApplyExtrinsicResult, MultiSignature, RuntimeAppPublic,
};

use sp_staking::SessionIndex;
use sp_std::prelude::*;
#[cfg(feature = "std")]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;

// A few exports that help ease life for downstream crates.
use frame_support::sp_runtime::traits::Convert;
use frame_support::sp_runtime::Perquintill;
use frame_support::traits::EqualPrivilegeOnly;
use frame_support::traits::SortedMembers;
use frame_support::weights::constants::WEIGHT_PER_MILLIS;
use frame_support::PalletId;
pub use frame_support::{
    construct_runtime, parameter_types,
    traits::{
        Currency, EstimateNextNewSession, Imbalance, KeyOwnerProofSystem, LockIdentifier,
        OnUnbalanced, Randomness, U128CurrencyToVote, ValidatorSet,
    },
    weights::{
        constants::{BlockExecutionWeight, ExtrinsicBaseWeight, RocksDbWeight, WEIGHT_PER_SECOND},
        IdentityFee, Weight,
    },
    StorageValue,
};
use frame_system::{EnsureRoot, EnsureSignedBy};
use primitives::{ApiError as AlephApiError, AuthorityId as AlephId};

pub use pallet_balances::Call as BalancesCall;
pub use pallet_timestamp::Call as TimestampCall;
use pallet_transaction_payment::{CurrencyAdapter, Multiplier, MultiplierUpdate};
use sp_consensus_aura::SlotDuration;
use sp_runtime::traits::{One, Zero};
#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;
pub use sp_runtime::{Perbill, Permill};

/// An index to a block.
pub type BlockNumber = u32;

/// Alias to 512-bit hash when used in the context of a transaction signature on the chain.
pub type Signature = MultiSignature;

/// Some way of identifying an account on the chain. We intentionally make it equivalent
/// to the public key of our transaction signing scheme.
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

/// The type for looking up accounts. We don't expect more than 4 billion of them, but you
/// never know...
pub type AccountIndex = u32;

/// Balance of an account.
pub type Balance = primitives::Balance;

/// Index of a transaction in the chain.
pub type Index = u32;

/// A hash of some data used by the chain.
pub type Hash = sp_core::H256;

/// Opaque types. These are used by the CLI to instantiate machinery that don't need to know
/// the specifics of the runtime. They can then be made to be agnostic over specific formats
/// of data like extrinsics, allowing for them to continue syncing the network through upgrades
/// to even the core data structures.
pub mod opaque {
    use super::*;

    pub use sp_runtime::OpaqueExtrinsic as UncheckedExtrinsic;

    /// Opaque block header type.
    pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
    /// Opaque block type.
    pub type Block = generic::Block<Header, UncheckedExtrinsic>;
    /// Opaque block identifier type.
    pub type BlockId = generic::BlockId<Block>;

    impl_opaque_keys! {
        pub struct SessionKeys {
            pub aura: Aura,
            pub aleph: Aleph,
        }
    }
}

pub const VERSION: RuntimeVersion = RuntimeVersion {
    spec_name: create_runtime_str!("aleph-node"),
    impl_name: create_runtime_str!("aleph-node"),
    authoring_version: 1,
    spec_version: 9,
    impl_version: 1,
    apis: RUNTIME_API_VERSIONS,
    transaction_version: 4,
};

/// This determines the average expected block time that we are targetting.
/// Blocks will be produced at a minimum duration defined by `SLOT_DURATION`.
/// `SLOT_DURATION` is picked up by `pallet_timestamp` which is in turn picked
/// up by `pallet_aura` to implement `fn slot_duration()`.
///
/// Change this to adjust the block time.
pub const MILLISECS_PER_BLOCK: u64 = 1000;

pub const SLOT_DURATION: u64 = MILLISECS_PER_BLOCK;

// Time is measured by number of blocks.
pub const MINUTES: BlockNumber = 60_000 / (MILLISECS_PER_BLOCK as BlockNumber);
pub const HOURS: BlockNumber = MINUTES * 60;
pub const DAYS: BlockNumber = HOURS * 24;

/// The version information used to identify this runtime when compiled natively.
#[cfg(feature = "std")]
pub fn native_version() -> NativeVersion {
    NativeVersion {
        runtime_version: VERSION,
        can_author_with: Default::default(),
    }
}

const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);
// The whole process for a single block should take 1s, of which 400ms is for creation,
// 200ms for propagation and 400ms for validation. Hence the block weight should be within 400ms.
const MAX_BLOCK_WEIGHT: Weight = 400 * WEIGHT_PER_MILLIS;
// We agreed to 5MB as the block size limit.
pub const MAX_BLOCK_SIZE: u32 = 5 * 1024 * 1024;

parameter_types! {
    pub const Version: RuntimeVersion = VERSION;
    pub const BlockHashCount: BlockNumber = 2400;
    pub BlockWeights: frame_system::limits::BlockWeights = frame_system::limits::BlockWeights
        ::with_sensible_defaults(MAX_BLOCK_WEIGHT, NORMAL_DISPATCH_RATIO);
    pub BlockLength: frame_system::limits::BlockLength = frame_system::limits::BlockLength
        ::max_with_normal_ratio(MAX_BLOCK_SIZE, NORMAL_DISPATCH_RATIO);
    pub const SS58Prefix: u8 = 42;
}

// Configure FRAME pallets to include in runtime.

impl frame_system::Config for Runtime {
    /// The basic call filter to use in dispatchable.
    type BaseCallFilter = frame_support::traits::Everything;
    /// Block & extrinsics weights: base values and limits.
    type BlockWeights = BlockWeights;
    /// The maximum length of a block (in bytes).
    type BlockLength = BlockLength;
    /// The identifier used to distinguish between accounts.
    type AccountId = AccountId;
    /// The aggregated dispatch type that is available for extrinsics.
    type Call = Call;
    /// The lookup mechanism to get account ID from whatever is passed in dispatchers.
    type Lookup = AccountIdLookup<AccountId, ()>;
    /// The index type for storing how many extrinsics an account has signed.
    type Index = Index;
    /// The index type for blocks.
    type BlockNumber = BlockNumber;
    /// The type for hashing blocks and tries.
    type Hash = Hash;
    /// The hashing algorithm used.
    type Hashing = BlakeTwo256;
    /// The header type.
    type Header = generic::Header<BlockNumber, BlakeTwo256>;
    /// The ubiquitous event type.
    type Event = Event;
    /// The ubiquitous origin type.
    type Origin = Origin;
    /// Maximum number of block number to block hash mappings to keep (oldest pruned first).
    type BlockHashCount = BlockHashCount;
    /// The weight of database operations that the runtime can invoke.
    type DbWeight = RocksDbWeight;
    /// Version of the runtime.
    type Version = Version;
    /// Converts a module to the index of the module in `construct_runtime!`.
    ///
    /// This type is being generated by `construct_runtime!`.
    type PalletInfo = PalletInfo;
    /// What to do if a new account is created.
    type OnNewAccount = ();
    /// What to do if an account is fully reaped from the system.
    type OnKilledAccount = ();
    /// The data to be stored in an account.
    type AccountData = pallet_balances::AccountData<Balance>;
    /// Weight information for the extrinsics of this pallet.
    type SystemWeightInfo = ();
    /// This is used as an identifier of the chain. 42 is the generic substrate prefix.
    type SS58Prefix = SS58Prefix;
    type OnSetCode = ();
}

parameter_types! {
    // https://github.com/paritytech/polkadot/blob/9ce5f7ef5abb1a4291454e8c9911b304d80679f9/runtime/polkadot/src/lib.rs#L784
    pub const MaxAuthorities: u32 = 100_000;
}

impl pallet_aura::Config for Runtime {
    type MaxAuthorities = MaxAuthorities;
    type AuthorityId = AuraId;
    type DisabledValidators = ();
}

parameter_types! {
    pub const UncleGenerations: BlockNumber = 0;
}

impl pallet_authorship::Config for Runtime {
    type FindAuthor = pallet_session::FindAccountFromAuthorIndex<Self, Aura>;
    type UncleGenerations = UncleGenerations;
    type FilterUncle = ();
    type EventHandler = (Staking,);
}

pub struct MinimumPeriod;
impl MinimumPeriod {
    pub fn get() -> u64 {
        Elections::millisecs_per_block() / 2
    }
}
impl<I: From<u64>> ::frame_support::traits::Get<I> for MinimumPeriod {
    fn get() -> I {
        I::from(Self::get())
    }
}

parameter_types! {
    pub const ExistentialDeposit: u128 = 500;
    pub const MaxLocks: u32 = 50;
}

impl pallet_balances::Config for Runtime {
    type MaxLocks = MaxLocks;
    type MaxReserves = ();
    type ReserveIdentifier = [u8; 8];
    /// The type for recording an account's balance.
    type Balance = Balance;
    /// The ubiquitous event type.
    type Event = Event;
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = pallet_balances::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
    pub const TransactionByteFee: Balance = 1;
    // This value increases the priority of `Operational` transactions by adding
    // a "virtual tip" that's equal to the `OperationalFeeMultiplier * final_fee`.
    // follows polkadot : https://github.com/paritytech/polkadot/blob/9ce5f7ef5abb1a4291454e8c9911b304d80679f9/runtime/polkadot/src/lib.rs#L369
    pub const OperationalFeeMultiplier: u8 = 5;
}

pub struct ConstantFeeMultiplierUpdate;

impl Convert<Multiplier, Multiplier> for ConstantFeeMultiplierUpdate {
    fn convert(m: Multiplier) -> Multiplier {
        m
    }
}

impl MultiplierUpdate for ConstantFeeMultiplierUpdate {
    fn min() -> Multiplier {
        Multiplier::one()
    }

    fn target() -> Perquintill {
        Default::default()
    }

    fn variability() -> Multiplier {
        Multiplier::zero()
    }
}

type NegativeImbalance = <Balances as Currency<AccountId>>::NegativeImbalance;

pub struct EverythingToTheTreasury;
impl OnUnbalanced<NegativeImbalance> for EverythingToTheTreasury {
    fn on_unbalanceds<B>(mut fees_then_tips: impl Iterator<Item = NegativeImbalance>) {
        if let Some(fees) = fees_then_tips.next() {
            Treasury::on_unbalanced(fees);
            if let Some(tips) = fees_then_tips.next() {
                Treasury::on_unbalanced(tips);
            }
        }
    }
}

impl pallet_transaction_payment::Config for Runtime {
    type OnChargeTransaction = CurrencyAdapter<Balances, EverythingToTheTreasury>;
    type TransactionByteFee = TransactionByteFee;
    type WeightToFee = IdentityFee<Balance>;
    type FeeMultiplierUpdate = ConstantFeeMultiplierUpdate;
    type OperationalFeeMultiplier = OperationalFeeMultiplier;
}

parameter_types! {
    pub MaximumSchedulerWeight: Weight = Perbill::from_percent(80) * BlockWeights::get().max_block;
    pub const MaxScheduledPerBlock: u32 = 50;
}

impl pallet_scheduler::Config for Runtime {
    type Event = Event;
    type Origin = Origin;
    type PalletsOrigin = OriginCaller;
    type Call = Call;
    type MaximumWeight = MaximumSchedulerWeight;
    type ScheduleOrigin = frame_system::EnsureRoot<AccountId>;
    type MaxScheduledPerBlock = MaxScheduledPerBlock;
    type WeightInfo = pallet_scheduler::weights::SubstrateWeight<Runtime>;
    type OriginPrivilegeCmp = EqualPrivilegeOnly;
}

impl pallet_sudo::Config for Runtime {
    type Event = Event;
    type Call = Call;
}

impl pallet_aleph::Config for Runtime {
    type AuthorityId = AlephId;
}

impl_opaque_keys! {
    pub struct SessionKeys {
        pub aura: Aura,
        pub aleph: Aleph,
    }
}

parameter_types! {
    pub const Offset: u32 = 0;
}

parameter_types! {
    pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(30);
}

pub struct MillisecsPerBlock;

impl MillisecsPerBlock {
    pub fn get() -> u64 {
        Elections::millisecs_per_block()
    }
}

impl<I: From<u64>> ::frame_support::traits::Get<I> for MillisecsPerBlock {
    fn get() -> I {
        I::from(Self::get())
    }
}

impl pallet_elections::Config for Runtime {
    type Event = Event;
    type DataProvider = Staking;
}

impl pallet_randomness_collective_flip::Config for Runtime {}

pub struct SessionPeriod;

impl SessionPeriod {
    pub fn get() -> u32 {
        Elections::session_period()
    }
}

impl<I: From<u32>> ::frame_support::traits::Get<I> for SessionPeriod {
    fn get() -> I {
        I::from(Self::get())
    }
}

impl pallet_session::Config for Runtime {
    type Event = Event;
    type ValidatorId = <Self as frame_system::Config>::AccountId;
    type ValidatorIdOf = pallet_staking::StashOf<Self>;
    type ShouldEndSession = pallet_session::PeriodicSessions<SessionPeriod, Offset>;
    type NextSessionRotation = pallet_session::PeriodicSessions<SessionPeriod, Offset>;
    type SessionManager = pallet_session::historical::NoteHistoricalRoot<Self, Staking>;
    type SessionHandler = <SessionKeys as OpaqueKeys>::KeyTypeIdProviders;
    type Keys = SessionKeys;
    type WeightInfo = pallet_session::weights::SubstrateWeight<Runtime>;
}

impl pallet_session::historical::Config for Runtime {
    type FullIdentification = pallet_staking::Exposure<AccountId, Balance>;
    type FullIdentificationOf = pallet_staking::ExposureOf<Runtime>;
}

parameter_types! {
    pub const BondingDuration: EraIndex = 14;
    pub const SlashDeferDuration: EraIndex = 13;
    pub const MaxNominatorRewardedPerValidator: u32 = 512;
    pub const OffendingValidatorsThreshold: Perbill = Perbill::from_percent(33);
}

pub struct SessionsPerEra;

impl SessionsPerEra {
    pub fn get() -> SessionIndex {
        Elections::sessions_per_era()
    }
}

impl<I: From<SessionIndex>> ::frame_support::traits::Get<I> for SessionsPerEra {
    fn get() -> I {
        I::from(Self::get())
    }
}

pub struct UniformEraPayout {}

impl pallet_staking::EraPayout<Balance> for UniformEraPayout {
    fn era_payout(_: Balance, _: Balance, _: u64) -> (Balance, Balance) {
        let miliseconds_per_era = Elections::millisecs_per_block()
            * Elections::session_period() as u64
            * Elections::sessions_per_era() as u64;
        primitives::staking::era_payout(miliseconds_per_era)
    }
}

impl pallet_staking::Config for Runtime {
    // Do not change this!!! It guarantees that we have DPoS instead of NPoS.
    const MAX_NOMINATIONS: u32 = 1;
    type Currency = Balances;
    type UnixTime = Timestamp;
    type CurrencyToVote = U128CurrencyToVote;
    type ElectionProvider = Elections;
    type GenesisElectionProvider = Elections;
    type RewardRemainder = Treasury;
    type Event = Event;
    type Slash = Treasury;
    type Reward = ();
    type SessionsPerEra = SessionsPerEra;
    type BondingDuration = BondingDuration;
    type SlashDeferDuration = SlashDeferDuration;
    type SlashCancelOrigin = EnsureRoot<AccountId>;
    type SessionInterface = Self;
    type EraPayout = UniformEraPayout;
    type NextNewSession = Session;
    type MaxNominatorRewardedPerValidator = MaxNominatorRewardedPerValidator;
    type OffendingValidatorsThreshold = OffendingValidatorsThreshold;
    type SortedListProvider = pallet_staking::UseNominatorsMap<Runtime>;
    type WeightInfo = pallet_staking::weights::SubstrateWeight<Runtime>;
}

impl pallet_timestamp::Config for Runtime {
    /// A timestamp: milliseconds since the unix epoch.
    type Moment = u64;
    type OnTimestampSet = Aura;
    type MinimumPeriod = MinimumPeriod;
    type WeightInfo = ();
}

impl<C> frame_system::offchain::SendTransactionTypes<C> for Runtime
where
    Call: From<C>,
{
    type Extrinsic = UncheckedExtrinsic;
    type OverarchingCall = Call;
}

parameter_types! {
    pub const MinVestedTransfer: Balance = 1_000_000;
}

impl pallet_vesting::Config for Runtime {
    type Event = Event;
    type Currency = Balances;
    type BlockNumberToBalance = ConvertInto;
    type MinVestedTransfer = MinVestedTransfer;
    type WeightInfo = pallet_vesting::weights::SubstrateWeight<Runtime>;
    // Maximum number of vesting schedules an account may have at a given moment
    // follows polkadot https://github.com/paritytech/polkadot/blob/9ce5f7ef5abb1a4291454e8c9911b304d80679f9/runtime/polkadot/src/lib.rs#L980
    const MAX_VESTING_SCHEDULES: u32 = 28;
}

pub const MILLICENTS: Balance = 100_000_000;
pub const CENTS: Balance = 1_000 * MILLICENTS; // 10^12 is one token, which for now is worth $0.1

// at a fixed cost $0.01 per byte, the constants are selected so that
// the base cost of starting a multisig action is $5
pub const ALLOCATION_COST: Balance = 412 * CENTS;
pub const BYTE_COST: Balance = CENTS;

pub const fn deposit(items: u32, bytes: u32) -> Balance {
    (items as Balance) * ALLOCATION_COST + (bytes as Balance) * BYTE_COST
}

parameter_types! {
    // One storage item; key size is 32; value is size 4+4+16+32 bytes = 56 bytes.
    pub const DepositBase: Balance = deposit(1, 88);
    // Additional storage item size of 32 bytes.
    pub const DepositFactor: Balance = deposit(0, 32);
    pub const MaxSignatories: u16 = 100;
}

impl pallet_multisig::Config for Runtime {
    type Event = Event;
    type Call = Call;
    type Currency = Balances;
    type DepositBase = DepositBase;
    type DepositFactor = DepositFactor;
    type MaxSignatories = MaxSignatories;
    type WeightInfo = pallet_multisig::weights::SubstrateWeight<Runtime>;
}

// We do not burn any money within treasury.
pub const TREASURY_BURN: u32 = 0;
// The percentage of the amount of the proposal that the proposer should deposit.
// We agreed on non-progressive deposit.
pub const TREASURY_PROPOSAL_BOND: u32 = 0;
// The proposer should deposit max{`TREASURY_PROPOSAL_BOND`% of the proposal value, $10}.
pub const TREASURY_MINIMUM_BOND: Balance = 1000 * CENTS;
// Every 4h we implement accepted proposals.
pub const TREASURY_SPEND_PERIOD: BlockNumber = 4 * HOURS;
// We allow at most 20 approvals in the queue at once.
pub const TREASURY_MAX_APPROVALS: u32 = 20;

parameter_types! {
    pub const Burn: Permill = Permill::from_percent(TREASURY_BURN);
    pub const ProposalBond: Permill = Permill::from_percent(TREASURY_PROPOSAL_BOND);
    pub const ProposalBondMinimum: Balance = TREASURY_MINIMUM_BOND;
    pub const MaxApprovals: u32 = TREASURY_MAX_APPROVALS;
    pub const SpendPeriod: BlockNumber = TREASURY_SPEND_PERIOD;
    pub const TreasuryPalletId: PalletId = PalletId(*b"a0/trsry");
}

pub struct TreasuryGovernance;
impl SortedMembers<AccountId> for TreasuryGovernance {
    fn sorted_members() -> Vec<AccountId> {
        vec![pallet_sudo::Pallet::<Runtime>::key()]
    }
}

impl pallet_treasury::Config for Runtime {
    type ApproveOrigin = EnsureSignedBy<TreasuryGovernance, AccountId>;
    type Burn = Burn;
    type BurnDestination = ();
    type Currency = Balances;
    type Event = Event;
    type MaxApprovals = MaxApprovals;
    type OnSlash = ();
    type PalletId = TreasuryPalletId;
    type ProposalBond = ProposalBond;
    type ProposalBondMinimum = ProposalBondMinimum;
    type RejectOrigin = EnsureSignedBy<TreasuryGovernance, AccountId>;
    type SpendFunds = ();
    type SpendPeriod = SpendPeriod;
    type WeightInfo = pallet_treasury::weights::SubstrateWeight<Runtime>;
}

impl pallet_utility::Config for Runtime {
    type Event = Event;
    type Call = Call;
    type WeightInfo = pallet_utility::weights::SubstrateWeight<Runtime>;
    type PalletsOrigin = OriginCaller;
}

// Create the runtime by composing the FRAME pallets that were previously configured.
construct_runtime!(
    pub enum Runtime where
        Block = Block,
        NodeBlock = opaque::Block,
        UncheckedExtrinsic = UncheckedExtrinsic
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>} = 0,
        RandomnessCollectiveFlip: pallet_randomness_collective_flip::{Pallet, Storage} = 1,
        Scheduler: pallet_scheduler::{Pallet, Call, Storage, Event<T>} = 2,
        Aura: pallet_aura::{Pallet, Config<T>} = 3,
        Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent} = 4,
        Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>} = 5,
        TransactionPayment: pallet_transaction_payment::{Pallet, Storage} = 6,
        Authorship: pallet_authorship::{Pallet, Call, Storage} = 7,
        Staking: pallet_staking::{Pallet, Call, Storage, Config<T>, Event<T>} = 8,
        History: pallet_session::historical::{Pallet} = 9,
        Session: pallet_session::{Pallet, Call, Storage, Event, Config<T>} = 10,
        Aleph: pallet_aleph::{Pallet, Storage} = 11,
        Elections: pallet_elections::{Pallet, Call, Storage, Config<T>, Event<T>} = 12,
        Treasury: pallet_treasury::{Pallet, Call, Storage, Config, Event<T>} = 13,
        Vesting: pallet_vesting::{Pallet, Call, Storage, Event<T>, Config<T>} = 14,
        Utility: pallet_utility::{Pallet, Call, Storage, Event} = 15,
        Multisig: pallet_multisig::{Pallet, Call, Storage, Event<T>} = 16,
        Sudo: pallet_sudo::{Pallet, Call, Config<T>, Storage, Event<T>} = 17,
    }
);

/// The address format for describing accounts.
pub type Address = sp_runtime::MultiAddress<AccountId, ()>;
/// Block header type as expected by this runtime.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
/// Block type as expected by this runtime.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;
/// A Block signed with a Justification
pub type SignedBlock = generic::SignedBlock<Block>;
/// BlockId type as expected by this runtime.
pub type BlockId = generic::BlockId<Block>;
/// The SignedExtension to the basic transaction logic.
pub type SignedExtra = (
    frame_system::CheckSpecVersion<Runtime>,
    frame_system::CheckTxVersion<Runtime>,
    frame_system::CheckGenesis<Runtime>,
    frame_system::CheckEra<Runtime>,
    frame_system::CheckNonce<Runtime>,
    frame_system::CheckWeight<Runtime>,
    pallet_transaction_payment::ChargeTransactionPayment<Runtime>,
);
/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic = generic::UncheckedExtrinsic<Address, Call, Signature, SignedExtra>;
/// Extrinsic type that has already been checked.
pub type CheckedExtrinsic = generic::CheckedExtrinsic<AccountId, Call, SignedExtra>;
/// Executive: handles dispatch to the various modules.
pub type Executive = frame_executive::Executive<
    Runtime,
    Block,
    frame_system::ChainContext<Runtime>,
    Runtime,
    AllPallets,
>;

impl_runtime_apis! {
    impl sp_api::Core<Block> for Runtime {
        fn version() -> RuntimeVersion {
            VERSION
        }

        fn execute_block(block: Block) {
            Executive::execute_block(block);
        }

        fn initialize_block(header: &<Block as BlockT>::Header) {
            Executive::initialize_block(header)
        }
    }

    impl sp_api::Metadata<Block> for Runtime {
        fn metadata() -> OpaqueMetadata {
            OpaqueMetadata::new(Runtime::metadata().into())
        }
    }

    impl sp_block_builder::BlockBuilder<Block> for Runtime {
        fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> ApplyExtrinsicResult {
            Executive::apply_extrinsic(extrinsic)
        }

        fn finalize_block() -> <Block as BlockT>::Header {
            Executive::finalize_block()
        }

        fn inherent_extrinsics(data: sp_inherents::InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
            data.create_extrinsics()
        }

        fn check_inherents(
            block: Block,
            data: sp_inherents::InherentData,
        ) -> sp_inherents::CheckInherentsResult {
            data.check_extrinsics(&block)
        }
    }

    impl sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block> for Runtime {
        fn validate_transaction(
            source: TransactionSource,
            tx: <Block as BlockT>::Extrinsic,
            block_hash: <Block as BlockT>::Hash,
        ) -> TransactionValidity {
            Executive::validate_transaction(source, tx, block_hash)
        }
    }

    impl sp_consensus_aura::AuraApi<Block, AuraId> for Runtime {
        fn slot_duration() -> SlotDuration {
            SlotDuration::from_millis(Aura::slot_duration())
        }

        fn authorities() -> Vec<AuraId> {
            Aura::authorities().to_vec ()
        }
    }

    impl sp_offchain::OffchainWorkerApi<Block> for Runtime {
        fn offchain_worker(header: &<Block as BlockT>::Header) {
            Executive::offchain_worker(header)
        }
    }

    impl sp_session::SessionKeys<Block> for Runtime {
        fn generate_session_keys(seed: Option<Vec<u8>>) -> Vec<u8> {
            opaque::SessionKeys::generate(seed)
        }

        fn decode_session_keys(
            encoded: Vec<u8>,
        ) -> Option<Vec<(Vec<u8>, KeyTypeId)>> {
            opaque::SessionKeys::decode_into_raw_public_keys(&encoded)
        }
    }

    impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Index> for Runtime {
        fn account_nonce(account: AccountId) -> Index {
            System::account_nonce(account)
        }
    }

    impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance> for Runtime {
        fn query_info(
            uxt: <Block as BlockT>::Extrinsic,
            len: u32,
        ) -> pallet_transaction_payment_rpc_runtime_api::RuntimeDispatchInfo<Balance> {
            TransactionPayment::query_info(uxt, len)
        }
        fn query_fee_details(
            uxt: <Block as BlockT>::Extrinsic,
            len: u32,
        ) -> pallet_transaction_payment::FeeDetails<Balance> {
            TransactionPayment::query_fee_details(uxt, len)
        }
    }

    impl primitives::AlephSessionApi<Block> for Runtime {
        fn authorities() -> Vec<AlephId> {
            Aleph::authorities()
        }

        fn millisecs_per_block() -> u64 {
            Elections::millisecs_per_block()
        }

        fn session_period() -> u32 {
            Elections::session_period()
        }

        fn next_session_authorities() -> Result<Vec<AlephId>, AlephApiError> {
            Session::queued_keys()
                .iter()
                .map(|(_, key)| key.get(AlephId::ID).ok_or(AlephApiError::DecodeKey))
                .collect::<Result<Vec<AlephId>, AlephApiError>>()
        }
    }
}
