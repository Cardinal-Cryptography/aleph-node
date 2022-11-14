#![cfg_attr(not(feature = "std"), no_std)]

use ink_env::Hash;
use ink_prelude::{string::String, vec::Vec};
use ink_storage::{
    traits::{PackedLayout, SpreadAllocate, SpreadLayout},
    Mapping,
};
use openbrush::contracts::psp22::PSP22Error;
use scale::{Decode, Encode};
use snarcos_extension::{ProvingSystem, SnarcosError, VerificationKeyIdentifier};

use crate::merkle_tree::KinderBlender;

mod contract;
mod merkle_tree;

type Scalar = u64;
type Nullifier = Scalar;

/// Type of the value in the Merkle tree leaf.
type Note = Hash;
/// Type of the value in the Merkle tree root.
type MerkleRoot = Hash;

/// Short identifier of a registered token contract.
type TokenId = u16;
/// `arkworks` does not support serializing `u128` and thus we have to operate on `u64` amounts.
type TokenAmount = u64;

type Set<T> = Mapping<T, ()>;

/// Verification key identifier for the `deposit` relation (to be registered in `pallet_snarcos`).
const DEPOSIT_VK_IDENTIFIER: VerificationKeyIdentifier = [b'd', b'p', b's', b't'];
/// Verification key identifier for the `withdraw` relation (to be registered in `pallet_snarcos`).
const WITHDRAW_VK_IDENTIFIER: VerificationKeyIdentifier = [b'w', b't', b'h', b'd'];
/// The only supported proving system for now.
const SYSTEM: ProvingSystem = ProvingSystem::Groth16;

/// PSP22 standard selector for transferring on behalf.
const PSP22_TRANSFER_FROM_SELECTOR: [u8; 4] = [0x54, 0xb3, 0xc7, 0x6e];

#[derive(Eq, PartialEq, Debug, Decode, Encode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum BlenderError {
    /// Caller is missing some permission.
    InsufficientPermission,
    /// Merkle tree is full - no new notes can be created.
    TooManyNotes,

    /// Pallet returned an error (through chain extension).
    ChainExtension(SnarcosError),

    /// PSP22 related error (e.g. insufficient allowance).
    Psp22(PSP22Error),
    /// Environment error (e.g. non-existing token contract).
    InkEnv(String),

    /// This token id is already taken.
    TokenIdAlreadyRegistered,
    /// There is no registered token under this token id.
    TokenIdNotRegistered,
}

/// Temporary implementation of two-to-one hashing function.
#[derive(
    Clone, Eq, PartialEq, Default, Decode, Encode, PackedLayout, SpreadLayout, SpreadAllocate,
)]
#[cfg_attr(
    feature = "std",
    derive(scale_info::TypeInfo, ink_storage::traits::StorageLayout)
)]
struct MerkleHasher;
impl KinderBlender<Hash> for MerkleHasher {
    fn blend_kinder(left: &Hash, right: &Hash) -> Hash {
        left.as_ref()
            .iter()
            .cloned()
            .zip(right.as_ref().iter().cloned())
            .map(|(l, r)| l ^ r)
            .collect::<Vec<_>>()
            .as_slice()
            .try_into()
            .unwrap()
    }
}
