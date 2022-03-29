use super::*;

use crate::{
    types::{LightBlockStorage, LightClientOptionsStorage},
    Pallet as TendermintLightClient,
};
use frame_benchmarking::{account, benchmarks, benchmarks_instance_pallet, whitelisted_caller};
use frame_system::{Origin, RawOrigin};
use sp_runtime::traits::Bounded;

const SEED: u32 = 0;
// // existential deposit multiplier
// const ED_MULTIPLIER: u32 = 10;

// benchmarks_instance_pallet! {

//     // initialize_client {

//     //     // let origin = Origin::root();
//     //     // let options = LightClientOptionsStorage::default();
//     //     // let initial_block: LightBlockStorage = serde_json::from_str(mock::TRUSTED_BLOCK).unwrap();

//     // }: _(RawOrigin::Signed(origin.clone()), options, initial_block)
//     // verify {
//     //     // assert_eq!(Balances::<T, I>::free_balance(&recipient), transfer_amount);
//     // }

// }

benchmarks! {
    initialize_client {

        let caller = RawOrigin::Root; //whitelisted_caller();
        // let caller = Origin::root();
        let options = LightClientOptionsStorage::default ();

        let a_ = crate::mock::TRUSTED_BLOCK;

        let initial_block = todo! ();

    }: initialize_client(caller.clone(), options, initial_block)
    verify {

        assert_eq! (true, true);

    }

    impl_benchmark_test_suite!(
        TendermintLightClient,
        crate::mock::ExtBuilder::default().existential_deposit(256).build(),
        crate::mock::Test,
    );
}
