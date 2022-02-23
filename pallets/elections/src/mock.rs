#![cfg(test)]

use super::*;
use crate as pallet_elections;

use frame_election_provider_support::{data_provider, ElectionDataProvider, VoteWeight};
use frame_support::{
    construct_runtime, parameter_types, sp_io, traits::GenesisBuild, weights::RuntimeDbWeight,
};
use sp_core::H256;
use sp_runtime::{
    testing::{Header, TestXt},
    traits::IdentityLookup,
};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

construct_runtime!(
    pub enum Test where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
        Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
        Elections: pallet_elections::{Pallet, Call, Storage, Config<T>, Event<T>},
    }
);

pub(crate) type AccountId = u64;

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub BlockWeights: frame_system::limits::BlockWeights =
        frame_system::limits::BlockWeights::simple_max(1024);
    pub const TestDbWeight: RuntimeDbWeight = RuntimeDbWeight {
        read: 25,
        write: 100
    };
}

impl frame_system::Config for Test {
    type BaseCallFilter = frame_support::traits::Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type Origin = Origin;
    type Call = Call;
    type Index = u64;
    type BlockNumber = u64;
    type Hash = H256;
    type Hashing = sp_runtime::traits::BlakeTwo256;
    type AccountId = u64;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Header = Header;
    type Event = Event;
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
}

parameter_types! {
    pub const ExistentialDeposit: u128 = 1;
}

impl pallet_balances::Config for Test {
    type Balance = u128;
    type MaxReserves = ();
    type ReserveIdentifier = [u8; 8];
    type DustRemoval = ();
    type Event = Event;
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
    type MaxLocks = ();
}

impl<C> frame_system::offchain::SendTransactionTypes<C> for Test
where
    Call: From<C>,
{
    type Extrinsic = TestXt<Call, ()>;
    type OverarchingCall = Call;
}

impl Config for Test {
    type Event = Event;
    type DataProvider = StakingMock;
}

pub struct StakingMock;
impl ElectionDataProvider<AccountId, u64> for StakingMock {
    const MAXIMUM_VOTES_PER_VOTER: u32 = 1;

    fn targets(_maybe_max_len: Option<usize>) -> data_provider::Result<Vec<AccountId>> {
        Ok(Vec::new())
    }

    fn voters(
        _maybe_max_len: Option<usize>,
    ) -> data_provider::Result<Vec<(AccountId, VoteWeight, Vec<AccountId>)>> {
        Ok(Vec::new())
    }

    fn desired_targets() -> data_provider::Result<u32> {
        Ok(0)
    }

    fn next_election_prediction(_now: u64) -> u64 {
        0
    }
}

pub fn new_test_ext(members: Vec<AccountId>) -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::default()
        .build_storage::<Test>()
        .unwrap();

    let balances: Vec<_> = (0..members.len()).map(|i| (i as u64, 10_000_000)).collect();

    pallet_balances::GenesisConfig::<Test> { balances }
        .assimilate_storage(&mut t)
        .unwrap();

    let millisecs_per_block = 1000;
    let session_period = 5;
    let sessions_per_era = 3;
    crate::GenesisConfig::<Test> {
        members,
        millisecs_per_block,
        session_period,
        sessions_per_era,
    }
    .assimilate_storage(&mut t)
    .unwrap();

    t.into()
}
