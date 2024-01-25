//! Benchmarking suite for the chain extension.
//!
//! We make use of the FRAME benchmarking framework. Although it is dedicated specifically to FRAME pallets, with a few
//! tricks we can use it to benchmark our chain extension as well.
//!
//! # Tricks
//!
//! The benchmarking framework expects to see two things here:
//! - A `Config` trait that defines the pallet's configuration.
//! - A `Pallet` struct that implements the `Config` trait.
//!
//! Since we don't have a pallet, we have to provide these two things ourselves. We do this by defining two dummy items.
//! To avoid confusion outside this module, we reexport the `Pallet` as `ChainExtensionBenchmarking` type, which can be
//! then used in the runtime benchmarking setup.

use frame_benchmarking::v2::*;
use frame_support::{sp_runtime::traits::Hash, BoundedVec};
use pallet_vk_storage::{KeyHasher, VerificationKeys};
use sp_std::vec;

use crate::args::VerifyArgs;

/// Dummy trait that defines the pallet's configuration. Since `auto trait` is not stable yet, we just provide a full
/// blanket implementation for all types.
trait Config: pallet_vk_storage::Config {}
impl<T: pallet_vk_storage::Config> Config for T {}

/// Dummy pallet struct. The only thing that actually matters is that it is generic over some type `T` that implements
/// the `Config` trait.
pub struct Pallet<T> {
    _phantom: sp_std::marker::PhantomData<T>,
}

/// A type alias for the pallet struct. This is the type that should be used in the runtime benchmarking setup and
/// limits the confusion to this module only.
pub type ChainExtensionBenchmarking<T> = Pallet<T>;

fn get_verification_artifacts<T: pallet_vk_storage::Config>() -> (
    VerifyArgs,
    BoundedVec<u8, <T as pallet_vk_storage::Config>::MaximumKeyLength>,
) {
    let verification_key =
        include_bytes!("../../benchmark-resources/bench_artifact_5_10_vk").to_vec();
    (
        VerifyArgs {
            verification_key_hash: KeyHasher::hash(&verification_key),
            proof: include_bytes!("../../benchmark-resources/bench_artifact_5_10_proof").to_vec(),
            public_input: include_bytes!("../../benchmark-resources/bench_artifact_5_10_input")
                .to_vec(),
        },
        verification_key.try_into().unwrap(),
    )
}

#[benchmarks]
mod benchmarks {
    use scale::{Decode, Encode};

    use super::*;
    use crate::{args::VerifyArgs, backend::BackendExecutorT};

    /// Benchmark `verify` arguments decoding.
    #[benchmark]
    fn verify_read_args(
        // Check input length up to ~10MB
        x: Linear<0, 10_000_000>,
    ) {
        let args = VerifyArgs {
            verification_key_hash: Default::default(),
            proof: vec![1; (x / 2) as usize],
            public_input: vec![2; (x / 2) as usize],
        }
        .encode();

        #[block]
        {
            VerifyArgs::decode(&mut &args[..]).unwrap();
        }
    }

    /// Benchmark proof verification.
    ///
    /// Due to macro internals, we cannot name the benchmark just `verify`.
    #[benchmark]
    fn verify_proof() {
        let (args, verification_key) = get_verification_artifacts::<T>();
        VerificationKeys::<T>::insert(args.verification_key_hash, verification_key);

        #[block]
        {
            <T as BackendExecutorT>::verify(args).unwrap();
        }
    }
}
