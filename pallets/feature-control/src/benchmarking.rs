use frame_benchmarking::v2::*;
use frame_system::RawOrigin;
use sp_std::vec;

use crate::{Call, Config, Feature, Pallet};

#[benchmarks]
mod benchmarks {
    #[benchmark]
    fn enable() {
        #[extrinsic_call]
        _(RawOrigin::Root, Feature::OnChainVerifier);

        assert!(ActiveFeatures::<T>::contains_key(Feature::OnChainVerifier));
    }

    #[benchmark]
    fn disable() {
        Pallet::enable(RawOrigin::Root, Feature::OnChainVerifier);

        #[extrinsic_call]
        _(RawOrigin::Root, Feature::OnChainVerifier);

        assert!(ActiveFeatures::<T>::contains_key(Feature::OnChainVerifier));
    }

    impl_benchmark_test_suite!(
        Pallet,
        crate::tests::new_test_ext(),
        crate::tests::TestRuntime
    );
}
