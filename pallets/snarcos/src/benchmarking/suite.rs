use frame_benchmarking::{account, benchmarks, vec};
use frame_support::{traits::Get, BoundedVec};
use frame_system::RawOrigin;

use crate::{ProvingSystem::*, *};

const SEED: u32 = 41;
const IDENTIFIER: VerificationKeyIdentifier = [0; 4];

fn caller<T: Config>() -> RawOrigin<<T as frame_system::Config>::AccountId> {
    RawOrigin::Signed(account("caller", 0, SEED))
}

benchmarks! {

    store_key {
        let l in 1 .. T::MaximumVerificationKeyLength::get();
        let key = vec![0u8; l as usize];
    } : _(caller::<T>(), IDENTIFIER, key)

    verify_xor {
        let key = get_artifact!(Groth16, Xor, VerifyingKey);
        let proof = get_artifact!(Groth16, Xor, Proof);
        let input = get_artifact!(Groth16, Xor, PublicInput);

        let _ = VerificationKeys::<T>::insert(
            IDENTIFIER,
            BoundedVec::try_from(key).unwrap()
        );

    } : verify(caller::<T>(), IDENTIFIER, proof, input, Groth16)

    verify_linear_equation {
        let key = get_artifact!(Groth16, LinearEquation, VerifyingKey);
        let proof = get_artifact!(Groth16, LinearEquation, Proof);
        let input = get_artifact!(Groth16, LinearEquation, PublicInput);

        let _ = VerificationKeys::<T>::insert(
            IDENTIFIER,
            BoundedVec::try_from(key).unwrap()
        );

    } : verify(caller::<T>(), IDENTIFIER, proof, input, Groth16)

}
