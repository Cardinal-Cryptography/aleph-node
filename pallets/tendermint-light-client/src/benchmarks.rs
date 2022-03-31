use super::*;
use crate::Pallet as TendermintLightClient;
use frame_benchmarking::{account, benchmarks, benchmarks_instance_pallet, whitelisted_caller};
use frame_system::{Origin, RawOrigin};
use sp_runtime::traits::Bounded;

benchmarks! {
    benchmark_for_initialize_client {

        let i in 0 .. i32::max_value() as u32;

        let caller = RawOrigin::Root; //whitelisted_caller();
        let options = types::LightClientOptionsStorage::default ();

        let initial_block = types::LightBlockStorage::create (i as i32, i as i32, i as i32, i as i32);

    }: initialize_client(caller.clone(), options, initial_block.clone ())

    verify {
        // check if persisted
        let last_imported = TendermintLightClient::<T>::get_last_imported_hash();
        assert_eq!(
            initial_block.signed_header.commit.block_id.hash,
            last_imported
        );

        // assert_eq! (true, true);

    }

    // TODO : benchmark client update call

    impl_benchmark_test_suite!(
        TendermintLightClient,
        mock::new_genesis_storage (),
        mock::TestRuntime,
    );

}
