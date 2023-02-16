#![cfg_attr(not(feature = "std"), no_std)]

#[ink::contract(env = snarcos_extension::DefaultEnvironment)]
mod preimage {

    use ark_serialize::CanonicalSerialize;
    use ink::{
        prelude::{vec, vec::Vec},
        storage::Mapping,
    };
    use liminal_ark_relations::PreimageRelationWithPublicInput;
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

        /// Caller commits to a specific value by passing the field value to which it hashes
        /// `commitment` is a corresponding output from the Poseidon hash function.
        #[ink(message)]
        pub fn commit(&mut self, commitment: [u64; 4]) -> Result<(), PreimageContractError> {
            let caller = Self::env().caller();

            if self.commitments.contains(caller) {
                return Err(PreimageContractError::AlreadyCommited);
            }

            self.commitments.insert(caller, &commitment);
            Ok(())
        }

        /// Serialize with `ark-serialize::CanonicalSerialize`.
        fn serialize<T: CanonicalSerialize + ?Sized>(t: &T) -> Vec<u8> {
            let mut bytes = vec![0; t.serialized_size()];
            t.serialize(&mut bytes[..]).expect("Failed to serialize");
            bytes.to_vec()
        }

        #[ink(message)]
        pub fn reveal(&mut self, proof: Vec<u8>) -> Result<(), PreimageContractError> {
            let caller = Self::env().caller();

            let commitment = self
                .commitments
                .get(caller)
                .ok_or(PreimageContractError::NotCommited)?;

            let relation = PreimageRelationWithPublicInput::new(commitment);

            self.env()
                .extension()
                .verify(
                    VERIFYING_KEY_IDENTIFIER,
                    proof,
                    Self::serialize::<Vec<CircuitField>>(&relation.serialize_public_input()),
                    ProvingSystem::Groth16,
                )
                .map_err(PreimageContractError::CannotVerify)?;

            self.commitments.remove(caller);
            Ok(())
        }

        /// Caller removes his commitment if any
        #[ink(message)]
        pub fn uncommit(&mut self) -> Result<(), PreimageContractError> {
            let caller = Self::env().caller();
            self.commitments.remove(caller);
            Ok(())
        }

        /// returns caller commitment or None
        #[ink(message)]
        pub fn commitment(&mut self) -> Option<[u64; 4]> {
            let caller = Self::env().caller();
            self.commitments.get(caller)
        }
    }
}
