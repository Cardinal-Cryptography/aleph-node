pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use super::*;
    use std::marker::PhantomData;

    #[pallet::generate_store($visibility_of_trait_store trait Store)]
    pub struct Pallet<T>(PhantomData<T>);

    // #[pallet::storage]
    // #[pallet::getter(fn authorities)]
    // pub(super) type Authorities<T: Config> = StorageValue<_, Vec<T::AuthorityId>, ValueQuery>;

}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        println!("yep");
    }
}
