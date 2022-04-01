use super::*;
use crate::{mock, Pallet as TendermintLightClient};
use frame_benchmarking::{account, benchmarks, whitelisted_caller};
use frame_support::{assert_err, assert_ok, traits::Get};
use frame_system::{Origin, RawOrigin};

benchmarks! {
    benchmark_for_initialize_client {

        let v in 0 .. T::MaxVotesCount::get();
        let i in 0 .. 1024 as u32;

        let caller = RawOrigin::Root;
        let options = types::LightClientOptionsStorage::default ();

        let initial_block = types::LightBlockStorage::create (v as i32, i as i32, i as i32, i as i32);

    }: initialize_client(caller.clone(), options, initial_block.clone ())

    verify {
        // check if persisted
        let last_imported = TendermintLightClient::<T>::get_last_imported_hash();
        assert_eq!(
            initial_block.signed_header.commit.block_id.hash,
            last_imported
        );
    }

    // TODO :
    benchmark_for_update_client {

        let options = types::LightClientOptionsStorage::default();
        let initial_block: types::LightBlockStorage = serde_json::from_str(mock::TRUSTED_BLOCK).unwrap();

        assert_ok!(TendermintLightClient::<T>::initialize_client(
            RawOrigin::Root.into() ,
            options,
            initial_block.clone ()
        ));

        let caller: T::AccountId = whitelisted_caller();

    }: update_client(RawOrigin::Signed(caller.clone()), initial_block.clone ())

    verify {
        // check if persisted
        // let last_imported = TendermintLightClient::<T>::get_last_imported_hash();
        // assert_eq!(
        //     initial_block.signed_header.commit.block_id.hash,
        //     last_imported
        // );

        assert_eq! (true, true);
    }

    // TODO : benchmark update client with pruning

    impl_benchmark_test_suite!(
        TendermintLightClient,
        mock::new_genesis_storage (),
        mock::TestRuntime,
    );

}
