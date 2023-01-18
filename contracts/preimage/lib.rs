#![cfg_attr(not(feature = "std"), no_std)]

#[ink::contract(env = snarcos_extension::DefaultEnvironment)]
mod preimage {

    use ark_ff::BigInteger256;
    use ark_serialize::CanonicalSerialize;
    use ink::{prelude::vec::Vec, reflect::ContractEventBase, storage::Mapping};
    // use relations::PreimageRelation;
    use snarcos_extension::{ProvingSystem, VerificationKeyIdentifier};

    const VERIFYING_KEY_IDENTIFIER: VerificationKeyIdentifier = [b'p', b'i', b'm', b'g'];

    type CircuitField = ark_bls12_381::Fr;

    #[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum PreimageContractError {
        AlreadyCommited,
        NotCommited,
    }

    #[ink(storage)]
    pub struct Preimage {
        commitments: Mapping<AccountId, Vec<u8>>,
    }

    impl Preimage {
        #[ink(constructor)]
        pub fn new() -> Self {
            Self {
                commitments: Mapping::default(),
            }
        }

        #[ink(message)]
        pub fn commit(&mut self, hash: Vec<u8>) -> Result<(), PreimageContractError> {
            let caller = Self::env().caller();

            if self.commitments.contains(caller) {
                return Err(PreimageContractError::AlreadyCommited);
            }

            self.commitments.insert(caller, &hash);
            Ok(())
        }

        fn bytes_to_u64_little_endian(bytes: &[u8]) -> u64 {
            let mut res = 0;
            for i in 0..8 {
                res |= (bytes[7 - i] as u64) << (8 * i);
            }
            res
        }

        #[ink(message)]
        pub fn reveal(
            &mut self,
            hash_bytes: Vec<u8>,
            proof: Vec<u8>,
        ) -> Result<(), PreimageContractError> {
            let caller = Self::env().caller();

            if !self.commitments.contains(caller) {
                return Err(PreimageContractError::NotCommited);
            }

            let hash = CircuitField::new(BigInteger256::from(Self::bytes_to_u64_little_endian(
                &hash_bytes,
            )));

            // let public_input = PreimageRelation::with_public_input(hash);

            // TODO : verify

            self.commitments.remove(caller);
            Ok(())
        }
    }
}
