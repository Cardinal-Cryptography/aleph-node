#![cfg_attr(not(feature = "std"), no_std)]

use ink_lang as ink;

mod binary_tree;

#[ink::contract(env = snarcos_extension::DefaultEnvironment)]
mod blender {
    use core::ops::Not;

    #[allow(unused_imports)]
    use ink_env::*;
    use ink_storage::{traits::SpreadAllocate, Mapping};
    use scale::{Decode, Encode};

    use crate::binary_tree::BinaryTree;

    type Scalar = u64;
    type Nullifier = Scalar;

    type Note = Hash;
    type MerkleRoot = Hash;

    type TokenId = u16;

    type Set<T> = Mapping<T, ()>;

    #[derive(Eq, PartialEq, Debug, Decode, Encode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        InsufficientPermission,
        TokenIdAlreadyRegistered,
    }

    type Result<T> = core::result::Result<T, Error>;

    #[ink(storage)]
    #[derive(SpreadAllocate)]
    pub struct Blender {
        notes: BinaryTree<Note, 1024>,
        merkle_roots: Set<MerkleRoot>,
        nullifiers: Set<Nullifier>,

        accepted_tokens: Mapping<TokenId, AccountId>,
        boss: AccountId,
    }

    impl Blender {
        #[ink(constructor)]
        pub fn new() -> Self {
            ink_lang::utils::initialize_contract(|blender: &mut Self| {
                blender.boss = Self::env().caller();
            })
        }

        #[ink(message, selector = 9)]
        pub fn accepted_token(&self, token_id: TokenId) -> Option<AccountId> {
            self.accepted_tokens.get(token_id)
        }

        #[ink(message, selector = 10)]
        pub fn accept_new_token(
            &mut self,
            token_id: TokenId,
            token_address: AccountId,
        ) -> Result<()> {
            self.ensure_mr_boss()?;
            self.accepted_tokens
                .contains(token_id)
                .not()
                .then(|| self.accepted_tokens.insert(token_id, &token_address))
                .ok_or(Error::TokenIdAlreadyRegistered)
        }

        fn ensure_mr_boss(&self) -> Result<()> {
            (self.env().caller() == self.boss)
                .then_some(())
                .ok_or(Error::InsufficientPermission)
        }
    }
}
