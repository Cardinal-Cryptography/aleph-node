use frame_support::{traits::Get, weights::Weight};
use sp_std::marker::PhantomData;

/// Weight functions for `pallet_bags_list`.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_bags_list::WeightInfo for WeightInfo<T> {
    // Storage: Staking Bonded (r:1 w:0)
    // Storage: Staking Ledger (r:1 w:0)
    // Storage: BagsList ListNodes (r:4 w:4)
    // Storage: BagsList ListBags (r:1 w:1)
    fn rebag_non_terminal() -> Weight {
        (64_669_000 as Weight)
            .saturating_add(T::DbWeight::get().reads(7 as Weight))
            .saturating_add(T::DbWeight::get().writes(5 as Weight))
    }
    // Storage: Staking Bonded (r:1 w:0)
    // Storage: Staking Ledger (r:1 w:0)
    // Storage: BagsList ListNodes (r:3 w:3)
    // Storage: BagsList ListBags (r:2 w:2)
    fn rebag_terminal() -> Weight {
        (62_211_000 as Weight)
            .saturating_add(T::DbWeight::get().reads(7 as Weight))
            .saturating_add(T::DbWeight::get().writes(5 as Weight))
    }
}
