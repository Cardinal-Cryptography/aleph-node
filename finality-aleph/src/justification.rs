use crate::{AuthorityKeystore, AuthoritySignature};
use codec::{Decode, Encode};
use sp_api::BlockT;

#[derive(Clone, Encode, Decode, PartialEq, Eq, Debug)]
pub struct AlephJustification {
    pub(crate) signature: AuthoritySignature,
}

impl AlephJustification {
    pub fn new<Block: BlockT>(auth_crypto_store: &AuthorityKeystore, hash: Block::Hash) -> Self {
        Self {
            signature: auth_crypto_store.sign(&hash.encode()[..]),
        }
    }
}
