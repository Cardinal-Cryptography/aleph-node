#![cfg_attr(not(feature = "std"), no_std)]

#[ink::contract(env = snarcos_extension::DefaultEnvironment)]
mod preimage {

    use ark_ff::BigInteger256;
    use ark_serialize::CanonicalSerialize;
    use ink::{
        prelude::{vec, vec::Vec},
        storage::Mapping,
    };
    use liminal_ark_relations::{GetPublicInput, PreimageRelation};
    use snarcos_extension::{ProvingSystem, SnarcosError, VerificationKeyIdentifier};

    const VERIFYING_KEY_IDENTIFIER: VerificationKeyIdentifier = [b'p', b'i', b'm', b'g'];

    type CircuitField = ark_bls12_381::Fr;

    #[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum PreimageContractError {
        AlreadyCommited,
        NotCommited,
        CannotVerify(SnarcosError),
    }

    #[ink(storage)]
    pub struct Preimage {
        commitments: Mapping<AccountId, [u64; 4]>,
    }

    impl Preimage {
        #[ink(constructor)]
        pub fn new() -> Self {
            Self {
                commitments: Mapping::default(),
            }
        }

        #[ink(message)]
        pub fn commit(&mut self, hash: [u64; 4]) -> Result<(), PreimageContractError> {
            let caller = Self::env().caller();

            if self.commitments.contains(caller) {
                return Err(PreimageContractError::AlreadyCommited);
            }

            self.commitments.insert(caller, &hash);
            Ok(())
        }

        /// Serialize with `ark-serialize::CanonicalSerialize`.
        fn serialize<T: CanonicalSerialize + ?Sized>(t: &T) -> Vec<u8> {
            let mut bytes = vec![0; t.serialized_size()];
            t.serialize(&mut bytes[..]).expect("Failed to serialize");
            bytes.to_vec()
        }

        #[ink(message)]
        pub fn reveal(
            &mut self,
            commitment: [u64; 4],
            proof: Vec<u8>,
        ) -> Result<(), PreimageContractError> {
            let caller = Self::env().caller();

            if !self.commitments.contains(caller) {
                return Err(PreimageContractError::NotCommited);
            }

            let hash = CircuitField::new(BigInteger256::new(commitment));
            let relation = PreimageRelation::with_public_input(hash);

            self.env()
                .extension()
                .verify(
                    VERIFYING_KEY_IDENTIFIER,
                    proof,
                    Self::serialize::<Vec<CircuitField>>(&relation.public_input()),
                    ProvingSystem::Groth16,
                )
                .map_err(|err: SnarcosError| PreimageContractError::CannotVerify(err))?;

            self.commitments.remove(caller);
            Ok(())
        }
    }
}
