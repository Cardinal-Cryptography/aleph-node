#![cfg_attr(not(feature = "std"), no_std)]

use ink_lang as ink;

mod merkle_tree;

#[ink::contract(env = snarcos_extension::DefaultEnvironment)]
#[allow(clippy::let_unit_value)]
mod blender {
    use core::ops::Not;

    use ark_serialize::CanonicalSerialize;
    use ink_env::call::{build_call, Call, ExecutionInput, Selector};
    #[allow(unused_imports)]
    use ink_env::*;
    use ink_prelude::{format, string::String, vec, vec::Vec};
    use ink_storage::{
        traits::{PackedLayout, SpreadAllocate, SpreadLayout},
        Mapping,
    };
    use openbrush::contracts::psp22::PSP22Error;
    use scale::{Decode, Encode};
    #[cfg(feature = "std")]
    use scale_info::TypeInfo;
    use snarcos_extension::{ProvingSystem, SnarcosError, VerificationKeyIdentifier};

    use crate::merkle_tree::{KinderBlender, MerkleTree};

    type Scalar = u64;
    type Nullifier = Scalar;

    type Note = Hash;
    type MerkleRoot = Hash;

    type TokenId = u16;
    type TokenAmount = u64;

    type Set<T> = Mapping<T, ()>;

    const DEPOSIT_VK_IDENTIFIER: VerificationKeyIdentifier = [b'd', b'p', b's', b't'];
    const WITHDRAW_VK_IDENTIFIER: VerificationKeyIdentifier = [b'w', b't', b'h', b'd'];
    const SYSTEM: ProvingSystem = ProvingSystem::Groth16;

    pub const PSP22_TRANSFER_FROM_SELECTOR: [u8; 4] = [0x54, 0xb3, 0xc7, 0x6e];

    #[derive(Eq, PartialEq, Debug, Decode, Encode)]
    #[cfg_attr(feature = "std", derive(TypeInfo))]
    pub enum Relation {
        Deposit,
        Withdraw,
    }

    #[derive(Eq, PartialEq, Debug, Decode, Encode)]
    #[cfg_attr(feature = "std", derive(TypeInfo))]
    pub enum Error {
        InsufficientPermission,
        TooMuchNotes,

        ChainExtension(SnarcosError),

        Psp22(PSP22Error),
        InkEnv(String),

        TokenIdAlreadyRegistered,
        TokenIdNotRegistered,
    }

    type Result<T> = core::result::Result<T, Error>;

    #[derive(
        Clone, Eq, PartialEq, Default, Decode, Encode, PackedLayout, SpreadLayout, SpreadAllocate,
    )]
    #[cfg_attr(feature = "std", derive(TypeInfo, ink_storage::traits::StorageLayout))]
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

    #[ink(storage)]
    #[derive(SpreadAllocate)]
    pub struct Blender {
        notes: MerkleTree<Note, MerkleHasher, 1024>,
        merkle_roots: Set<MerkleRoot>,
        nullifiers: Set<Nullifier>,

        registered_tokens: Mapping<TokenId, AccountId>,
        boss: AccountId,
    }

    impl Default for Blender {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Blender {
        #[ink(constructor)]
        pub fn new() -> Self {
            ink_lang::utils::initialize_contract(|blender: &mut Self| {
                blender.boss = Self::env().caller();
            })
        }

        #[ink(message, selector = 1)]
        pub fn deposit(
            &mut self,
            token_id: TokenId,
            value: TokenAmount,
            note: Note,
            proof: Vec<u8>,
        ) -> Result<()> {
            self.acquire_deposit(token_id, value)?;
            self.verify_deposit(token_id, value, note, proof)?;
            self.notes.add(note).map_err(|_| Error::TooMuchNotes)?;
            self.merkle_roots.insert(self.notes.root(), &());
            Ok(())
        }

        fn acquire_deposit(&self, token_id: TokenId, deposit: TokenAmount) -> Result<()> {
            let token_contract = self
                .registered_token_address(token_id)
                .ok_or(Error::TokenIdNotRegistered)?;

            build_call::<super::blender::Environment>()
                .call_type(Call::new().callee(token_contract))
                .exec_input(
                    ExecutionInput::new(Selector::new(PSP22_TRANSFER_FROM_SELECTOR))
                        .push_arg(self.env().caller())
                        .push_arg(self.env().account_id())
                        .push_arg(deposit)
                        .push_arg::<Vec<u8>>(vec![]),
                )
                .call_flags(CallFlags::default().set_allow_reentry(true))
                .returns::<core::result::Result<(), PSP22Error>>()
                .fire()
                .map_err(|e| Error::InkEnv(format!("{:?}", e)))?
                .map_err(Error::Psp22)
        }

        pub fn serialize<T: CanonicalSerialize + ?Sized>(t: &T) -> Vec<u8> {
            let mut bytes = vec![0; t.serialized_size()];
            t.serialize(&mut bytes[..]).expect("Failed to serialize");
            bytes.to_vec()
        }

        fn verify_deposit(
            &self,
            token_id: TokenId,
            value: TokenAmount,
            note: Note,
            proof: Vec<u8>,
        ) -> Result<()> {
            let serialized_input = [
                Self::serialize(&token_id),
                Self::serialize(&value),
                Self::serialize(note.as_ref()),
            ]
            .concat();

            self.env()
                .extension()
                .verify(DEPOSIT_VK_IDENTIFIER, proof, serialized_input, SYSTEM)
                .map_err(Error::ChainExtension)
        }

        #[ink(message, selector = 8)]
        pub fn register_vk(&mut self, relation: Relation, vk: Vec<u8>) -> Result<()> {
            self.ensure_mr_boss()?;
            let identifier = match relation {
                Relation::Deposit => DEPOSIT_VK_IDENTIFIER,
                Relation::Withdraw => WITHDRAW_VK_IDENTIFIER,
            };
            self.env()
                .extension()
                .store_key(identifier, vk)
                .map_err(Error::ChainExtension)
        }

        #[ink(message, selector = 9)]
        pub fn registered_token_address(&self, token_id: TokenId) -> Option<AccountId> {
            self.registered_tokens.get(token_id)
        }

        #[ink(message, selector = 10)]
        pub fn register_new_token(
            &mut self,
            token_id: TokenId,
            token_address: AccountId,
        ) -> Result<()> {
            self.ensure_mr_boss()?;
            self.registered_tokens
                .contains(token_id)
                .not()
                .then(|| self.registered_tokens.insert(token_id, &token_address))
                .ok_or(Error::TokenIdAlreadyRegistered)
        }

        fn ensure_mr_boss(&self) -> Result<()> {
            (self.env().caller() == self.boss)
                .then_some(())
                .ok_or(Error::InsufficientPermission)
        }
    }
}
