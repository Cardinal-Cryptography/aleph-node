use frame_benchmarking::{account, benchmarks, vec, Vec};
use frame_support::{traits::Get, BoundedVec};
use frame_system::RawOrigin;

use crate::{benchmarking::import::Artifacts, ProvingSystem::*, *};

const SEED: u32 = 41;
const IDENTIFIER: VerificationKeyIdentifier = [0; 4];

fn caller<T: Config>() -> RawOrigin<<T as frame_system::Config>::AccountId> {
    RawOrigin::Signed(account("caller", 0, SEED))
}

fn insert_key<T: Config>(key: Vec<u8>) {
    VerificationKeys::<T>::insert(IDENTIFIER, BoundedVec::try_from(key).unwrap());
}

benchmarks! {

    store_key {
        let l in 1 .. T::MaximumVerificationKeyLength::get();
        let key = vec![0u8; l as usize];
    } : _(caller::<T>(), IDENTIFIER, key)

    verify_xor {
        let Artifacts { key, proof, input } = get_artifacts!(Groth16, Xor);
        let _ = insert_key::<T>(key);
    } : verify(caller::<T>(), IDENTIFIER, proof, input, Groth16)

    verify_linear_equation {
        let Artifacts { key, proof, input } = get_artifacts!(Groth16, Xor);
        let _ = insert_key::<T>(key);
    } : verify(caller::<T>(), IDENTIFIER, proof, input, Groth16)

}
