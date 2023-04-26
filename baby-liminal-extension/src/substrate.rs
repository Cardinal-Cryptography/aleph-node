use obce::substrate::{
    frame_support::weights::Weight,
    frame_system::Config as SysConfig,
    pallet_contracts::{chain_extension::RetVal, Config as ContractConfig},
    sp_core::crypto::UncheckedFrom,
    sp_runtime::{traits::StaticLookup, AccountId32},
    sp_std::{mem::size_of, vec::Vec},
    ChainExtensionEnvironment, ExtensionContext,
};
use pallet_baby_liminal::{Config as BabyLiminalConfig, Error, KeyPairIdentifier, WeightInfo};
use primitives::host_functions::poseidon;

use crate::{
    executor::Executor, BabyLiminalError, BabyLiminalExtension, SingleHashInput,
    BABY_LIMINAL_STORE_KEY_PAIR_TOO_LONG_KEY_PAIR,
};

pub type ByteCount = u32;

/// Provides a weight of `store_key_pair` dispatchable.
pub fn weight_of_store_key_pair<T: BabyLiminalConfig>(
    proving_key_len: ByteCount,
    verification_key_len: ByteCount,
) -> Weight {
    <<T as BabyLiminalConfig>::WeightInfo as WeightInfo>::store_key_pair(
        proving_key_len,
        verification_key_len,
    )
}

#[derive(Default)]
pub struct Extension;

#[obce::implementation]
impl<'a, E, T, Env> BabyLiminalExtension for ExtensionContext<'a, E, T, Env, Extension>
where
    T: SysConfig + ContractConfig + BabyLiminalConfig,
    <<T as SysConfig>::Lookup as StaticLookup>::Source: From<<T as SysConfig>::AccountId>,
    <T as SysConfig>::AccountId: UncheckedFrom<<T as SysConfig>::Hash> + AsRef<[u8]>,
    Env: ChainExtensionEnvironment<E, T> + Executor<T>,
    <T as SysConfig>::RuntimeOrigin: From<Option<AccountId32>>,
{
    // We assume synthetic lengths for proving and verification keys, necessary to setup benchmarks
    // with a grid of parameters. The approximate key pair length measured in bytes is capped at
    // the sum of the caps for the proving (50,000) and verification (10,000) keys.
    #[obce(
        weight(
            expr = r#"{
                let approx_key_pair_len = env
                    .in_len()
                    .saturating_sub(size_of::<KeyPairIdentifier>() as ByteCount);

                if approx_key_pair_len > 60_000 {
                    return Ok(RetVal::Converging(BABY_LIMINAL_STORE_KEY_PAIR_TOO_LONG_KEY_PAIR));
                }

                let synthetic_verification_key_len = 1;
                let synthetic_proving_key_len = approx_key_pair_len.saturating_sub(
                    synthetic_verification_key_len as ByteCount
                );

                weight_of_store_key_pair::<T>(
                    synthetic_proving_key_len, synthetic_verification_key_len
                )
            }"#,
            pre_charge
        ),
        ret_val
    )]
    fn store_key_pair(
        &mut self,
        origin: AccountId32,
        identifier: KeyPairIdentifier,
        proving_key: Vec<u8>,
        verification_key: Vec<u8>,
    ) -> Result<(), BabyLiminalError> {
        let pre_charged = self.pre_charged().unwrap();

        // Now we know the exact key length.
        self.env.adjust_weight(
            pre_charged,
            weight_of_store_key_pair::<T>(
                proving_key.len() as ByteCount,
                verification_key.len() as ByteCount,
            ),
        );

        match Env::store_key_pair(origin, identifier, proving_key, verification_key) {
            Ok(_) => Ok(()),
            // In case `DispatchResultWithPostInfo` was returned (or some simpler equivalent for
            // `bare_store_key`), we could have adjusted weight. However, for the storing key action
            // it doesn't make much sense.
            Err(Error::ProvingKeyTooLong) => Err(BabyLiminalError::ProvingKeyTooLong),
            Err(Error::VerificationKeyTooLong) => Err(BabyLiminalError::VerificationKeyTooLong),
            Err(Error::IdentifierAlreadyInUse) => Err(BabyLiminalError::IdentifierAlreadyInUse),
            _ => Err(BabyLiminalError::StoreKeyPairErrorUnknown),
        }
    }

    #[obce(
        weight(
            expr = "<<T as BabyLiminalConfig>::WeightInfo as WeightInfo>::verify()",
            pre_charge
        ),
        ret_val
    )]
    fn verify(
        &mut self,
        identifier: KeyPairIdentifier,
        proof: Vec<u8>,
        input: Vec<u8>,
    ) -> Result<(), BabyLiminalError> {
        let pre_charged = self.pre_charged().unwrap();

        let result = Env::verify(identifier, proof, input);

        // In case the dispatchable failed and pallet provides us with post-dispatch weight, we can
        // adjust charging. Otherwise (positive case or no post-dispatch info) we cannot refund
        // anything.
        if let Err((_, Some(actual_weight))) = &result {
            self.env.adjust_weight(pre_charged, *actual_weight);
        };

        match result {
            Ok(_) => Ok(()),
            Err((Error::DeserializingProofFailed, _)) => {
                Err(BabyLiminalError::DeserializingProofFailed)
            }
            Err((Error::DeserializingPublicInputFailed, _)) => {
                Err(BabyLiminalError::DeserializingPublicInputFailed)
            }
            Err((Error::UnknownKeyPairIdentifier, _)) => {
                Err(BabyLiminalError::UnknownKeyPairIdentifier)
            }
            Err((Error::DeserializingVerificationKeyFailed, _)) => {
                Err(BabyLiminalError::DeserializingVerificationKeyFailed)
            }
            Err((Error::VerificationFailed(_), _)) => Err(BabyLiminalError::VerificationFailed),
            Err((Error::IncorrectProof, _)) => Err(BabyLiminalError::IncorrectProof),
            Err((_, _)) => Err(BabyLiminalError::VerifyErrorUnknown),
        }
    }

    #[obce(weight(
        expr = "<<T as BabyLiminalConfig>::WeightInfo as WeightInfo>::poseidon_one_to_one_host()",
        pre_charge
    ))]
    fn poseidon_one_to_one(&self, input: [SingleHashInput; 1]) -> SingleHashInput {
        poseidon::one_to_one_hash(input[0])
    }

    #[obce(weight(
        expr = "<<T as BabyLiminalConfig>::WeightInfo as WeightInfo>::poseidon_two_to_one_host()",
        pre_charge
    ))]
    fn poseidon_two_to_one(&self, input: [SingleHashInput; 2]) -> SingleHashInput {
        poseidon::two_to_one_hash(input[0], input[1])
    }

    #[obce(weight(
        expr = "<<T as BabyLiminalConfig>::WeightInfo as WeightInfo>::poseidon_four_to_one_host()",
        pre_charge
    ))]
    fn poseidon_four_to_one(&self, input: [SingleHashInput; 4]) -> SingleHashInput {
        poseidon::four_to_one_hash(input[0], input[1], input[2], input[3])
    }
}
