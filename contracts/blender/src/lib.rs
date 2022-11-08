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
    use snarcos_extension::{SnarcosError, VerificationKeyIdentifier};
    use sp_std::vec::Vec;

    use crate::{binary_tree::BinaryTree, blender::Error::ChainExtensionError};

    type Scalar = u64;
    type Nullifier = Scalar;

    type Note = Hash;
    type MerkleRoot = Hash;

    type TokenId = u16;
    type Balance = u128;

    type Set<T> = Mapping<T, ()>;

    const DEPOSIT_VK_IDENTIFIER: VerificationKeyIdentifier =
        ['d' as u8, 'p' as u8, 's' as u8, 't' as u8];
    const WITHDRAW_VK_IDENTIFIER: VerificationKeyIdentifier =
        ['w' as u8, 't' as u8, 'h' as u8, 'd' as u8];

    #[derive(Eq, PartialEq, Debug, Decode, Encode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Relation {
        Deposit,
        Withdraw,
    }

    #[derive(Eq, PartialEq, Debug, Decode, Encode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        InsufficientPermission,
        ChainExtensionError(SnarcosError),
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

        #[ink(message, selector = 1)]
        pub fn deposit(
            &mut self,
            value: Balance,
            token_id: TokenId,
            note: Note,
            proof: Vec<u8>,
        ) -> Result<()> {
            Ok(())
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
                .map_err(ChainExtensionError)
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
