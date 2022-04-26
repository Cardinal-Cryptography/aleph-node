use super::*;
use crate::{
    types::{LightClientOptionsStorage, TendermintHashStorage, TimestampStorage},
    Pallet as TendermintLightClient,
};
use frame_benchmarking::{benchmarks, whitelisted_caller};
use frame_support::{
    assert_ok, log,
    traits::{Get, OriginTrait, UnixTime},
};
use frame_system::{Pallet as System, RawOrigin};
use generator::generate_consecutive_blocks;
use pallet_timestamp::Pallet as Timestamp;
use scale_info::prelude::string::String;
use sp_runtime::traits::{AtLeast32Bit, Bounded, One, Zero};

fn mul<M>(value: M, by: u64) -> M
where
    M: AtLeast32Bit,
{
    let mut res = value;
    for _ in 0..by {
        res += M::one();
    }
    res
}

benchmarks! {
    // benchmark_for_initialize_client {

    //     let v in 1 .. 2;//T::MaxVotesCount::get();
    //     let mut blocks = generate_consecutive_blocks (1, String::from ("test-chain"), v, 3, TimestampStorage::new (3, 0));

    //     let initial_block = blocks.pop ().unwrap ();
    //     let options = LightClientOptionsStorage::default();

    // }: initialize_client(RawOrigin::Root, options, initial_block.clone ())

    //     verify {
    //         // check if persisted
    //         let last_imported = TendermintLightClient::<T>::get_last_imported_block_hash();
    //         assert_eq!(
    //             initial_block.signed_header.commit.block_id.hash,
    //             TendermintHashStorage::Some (last_imported)
    //         );
    //     }

    // benchmark_for_update_client {

    //     let v in 1 .. 2;//T::MaxVotesCount::get();
    //     let mut blocks = generate_consecutive_blocks (2, String::from ("test-chain"), v, 3, TimestampStorage::new (3, 0));

    //     let options = LightClientOptionsStorage::default();
    //     let initial_block = blocks.pop ().unwrap ();

    //     assert_ok!(TendermintLightClient::<T>::initialize_client(
    //         RawOrigin::Root.into() ,
    //         options,
    //         initial_block
    //     ));

    //     let caller: T::AccountId = whitelisted_caller();
    //     let untrusted_block = blocks.pop ().unwrap ();

    // }: update_client(RawOrigin::Signed(caller.clone()), untrusted_block.clone ())

    //     verify {
    //         // check if persisted
    //         let last_imported = TendermintLightClient::<T>::get_last_imported_block_hash();
    //         assert_eq!(
    //             untrusted_block.signed_header.commit.block_id.hash,
    //             TendermintHashStorage::Some (last_imported)
    //         );
    //     }

    // this benchmarks update_client call which causes pruning of the oldest block
    // mock runtime keeps 3 headers, therefore rollover happens after inserting a fourth consecutive block
    benchmark_for_update_client_with_pruning {

        let v in 1 .. 1; // T::MaxVotesCount::get();

        let mut block_number = System::<T>::block_number();
        System::<T>::set_block_number(block_number);

        let mut now = T::Moment::zero ();
        Timestamp::<T>::set_timestamp(now);

        // initial block at 1970-01-01T00:00:00Z
        let mut blocks = generate_consecutive_blocks ((T::HeadersToKeep::get() + 1u32) as usize, String::from ("test-chain"), v, 0, TimestampStorage::new (0, 0));

        let options = LightClientOptionsStorage::default();

        let initial_block = blocks.pop ().unwrap ();
        assert_ok!(TendermintLightClient::<T>::initialize_client(
            RawOrigin::Root.into(),
            options,
            initial_block.clone ()
        ));

        let caller: T::AccountId = whitelisted_caller();

        for _ in 1..T::HeadersToKeep::get() {

            now = now + mul(T::Moment::one (), 1000) ;
            block_number = block_number + One::one();

            System::<T>::set_block_number(block_number);
            Timestamp::<T>::set_timestamp(now);

            log::info!(target: "runtime-benchmarks::tendermint-lc", "now {:?}", T::TimeProvider::now ());

            let next = blocks.pop ().unwrap ();
            assert_ok!(TendermintLightClient::<T>::update_client(
                RawOrigin::Signed(caller.clone()).into (),
                next
            ));
        }

        let last = blocks.pop ().expect ("No more blocks!");

    }: update_client(RawOrigin::Signed(caller.clone()), last)

        verify {
            // check if rollover has happened
            let expected_last_imported_block_hash = TendermintLightClient::<T>::get_last_imported_block_hash();
            let last_imported_block_hash = TendermintLightClient::<T>::get_imported_hash(0).unwrap ();

            assert_eq!(
                expected_last_imported_block_hash,
                last_imported_block_hash,
                "This hash should have been pruned"
            );

            if let TendermintHashStorage::Some(hash) = initial_block.signed_header.commit.block_id.hash {
                assert_eq! (None, ImportedBlocks::<T>::get(hash), "This block should have been pruned");
            }

        }

    impl_benchmark_test_suite!(
        TendermintLightClient,
        crate::mock::new_genesis_storage (),
        crate::mock::TestRuntime,
    );

}
