use frame_benchmarking::{account, benchmarks, vec};
use frame_support::{traits::Get, BoundedVec};
use frame_system::RawOrigin;

use crate::{ProvingSystem::*, *};

const SEED: u32 = 41;

benchmarks! {

    store_key {
        let caller = account("caller", 0, SEED);
        let identifier = [0u8; 4];
        let l in 1 .. T::MaximumVerificationKeyLength::get();
        let key = vec![0u8; l as usize];
    } : _(RawOrigin::Signed(caller), identifier, key.clone())

    verify_xor {
        let caller = account("caller", 0, SEED);

        let key = get_artifact!(Groth16, Xor, VerifyingKey);
        let proof = get_artifact!(Groth16, Xor, Proof);
        let input = get_artifact!(Groth16, Xor, PublicInput);

        let identifier = [0u8; 4];
        let _ = VerificationKeys::<T>::insert(
            identifier.clone(),
            BoundedVec::try_from(key).unwrap()
        );

    } : verify(RawOrigin::Signed(caller), identifier, proof, input, Groth16)

    verify_linear_equation {
        let caller = account("caller", 0, SEED);

        let key = get_artifact!(Groth16, LinearEquation, VerifyingKey);
        let proof = get_artifact!(Groth16, LinearEquation, Proof);
        let input = get_artifact!(Groth16, LinearEquation, PublicInput);

        let identifier = [0u8; 4];
        let _ = VerificationKeys::<T>::insert(
            identifier.clone(),
            BoundedVec::try_from(key).unwrap()
        );

    } : verify(RawOrigin::Signed(caller), identifier, proof, input, Groth16)

}
