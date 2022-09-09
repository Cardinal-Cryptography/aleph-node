#![cfg_attr(not(feature = "std"), no_std)]

mod relation;

use frame_support::pallet_prelude::StorageVersion;
pub use pallet::*;

/// The current storage version.
const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

#[frame_support::pallet]
pub mod pallet {
    use ark_bls12_381::Bls12_381;
    use ark_groth16::Groth16;
    use ark_snark::SNARK;
    use ark_std::rand::{prelude::StdRng, SeedableRng};
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::OriginFor;

    use super::*;
    use crate::relation::XorRelation;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        ElSnarco,
    }

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    impl<T: Config> Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(41)]
        pub fn summon_el_snarco(_origin: OriginFor<T>) -> DispatchResult {
            let mut rng = StdRng::from_seed([0u8; 32]);
            let circuit = XorRelation::new(2, 3, 1);

            let (pk, vk) = Groth16::<Bls12_381>::circuit_specific_setup(circuit.clone(), &mut rng)
                .unwrap_or_else(|e| panic!("Problems with setup: {:?}", e));

            let public_input = [circuit.public_xoree.into()];

            let proof = Groth16::prove(&pk, circuit, &mut rng)
                .unwrap_or_else(|e| panic!("Cannot prove: {:?}", e));
            let valid_proof = Groth16::verify(&vk, &public_input, &proof)
                .unwrap_or_else(|e| panic!("Cannot verify: {:?}", e));

            ensure!(valid_proof, "Something is no yes");

            Self::deposit_event(Event::ElSnarco);
            Ok(())
        }
    }
}
