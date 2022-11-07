#![cfg_attr(not(feature = "std"), no_std)]

use ink_lang as ink;

mod binary_tree;

#[ink::contract(env = snarcos_extension::DefaultEnvironment)]
mod blender {
    use ink_storage::{traits::SpreadAllocate, Mapping};

    use crate::binary_tree::BinaryTree;

    type Scalar = u64;
    type Nullifier = Scalar;

    type Note = ink_env::Hash;
    type MerkleRoot = ink_env::Hash;

    type TokenId = u16;

    type Set<T> = Mapping<T, ()>;

    #[ink(storage)]
    #[derive(Default, SpreadAllocate)]
    pub struct Blender {
        notes: BinaryTree<Note, 1024>,
        merkle_roots: Set<MerkleRoot>,
        nullifiers: Set<Nullifier>,

        accepted_tokens: Mapping<TokenId, ink_env::AccountId>,
    }

    impl Blender {
        #[ink(constructor)]
        pub fn new() -> Self {
            ink_lang::utils::initialize_contract(|_| {})
        }

        #[ink(message)]
        pub fn nop(&self) {}
    }
}
