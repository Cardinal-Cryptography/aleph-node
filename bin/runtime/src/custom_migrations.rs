use sp_std::marker::PhantomData;

use super::*;
use frame_election_provider_support::SortedListProvider;
use frame_support::{log, traits::OnRuntimeUpgrade, weights::Weight};

pub struct UpgradeToBags<T>(PhantomData<T>);
impl<T: pallet_staking::Config> OnRuntimeUpgrade for UpgradeToBags<T> {
    fn on_runtime_upgrade() -> Weight {
        let pre_migrate_nominators_count = pallet_staking::Nominators::<T>::iter_keys().count();
        let nominators = pallet_staking::Nominators::<T>::iter_keys();
        let weight_of_fn = pallet_staking::Pallet::<T>::weight_of_fn();

        log::info!(
            target: "CustomMigration::UpgradeToBags",
            "Nominator count: {}",
            pre_migrate_nominators_count
        );
        let moved = T::SortedListProvider::regenerate(nominators, weight_of_fn);
        log::info!(target: "CustomMigration::UpgradeToBags", "Moved {} nominators", moved);

        let after_migrate_nominators_count = T::SortedListProvider::count() as usize;
        assert_eq!(pre_migrate_nominators_count, after_migrate_nominators_count);

        BlockWeights::get().max_block
    }
}
