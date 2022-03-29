use super::*;
use crate::{mock, types::*, Pallet as TendermintLightClient};
use frame_benchmarking::{account, benchmarks, benchmarks_instance_pallet, whitelisted_caller};
use frame_system::{Origin, RawOrigin};
use sp_runtime::traits::Bounded;
use std::mem;

benchmarks! {
    benchmark_for_initialize_client {

        let caller = RawOrigin::Root; //whitelisted_caller();
        let options = LightClientOptionsStorage::default ();

        // TODO : for all possible memory sizes of struct
        // TODO : benchmark for all Vec<T>
        // let size_ = mem::size_of::<LightBlockStorage>();
        let initial_block: LightBlockStorage = serde_json::from_str(mock::TRUSTED_BLOCK).unwrap();

    }: initialize_client(caller.clone(), options, initial_block)

    verify {
        // check if event fired
        assert_eq! (true, true);

    }

    // impl_benchmark_test_suite!(
    //     TendermintLightClient,
    //     crate::mock::ExtBuilder::default().existential_deposit(256).build(),
    //     crate::mock::Test,
    // );
}
