use super::{NodeCount, NodeIndex, SignatureSet};
use parity_scale_codec::{Decode, Encode};
use sp_core::crypto::KeyTypeId;
use sp_runtime::RuntimeAppPublic;

pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"alp0");

mod app {
    use sp_application_crypto::{app_crypto, ed25519};
    app_crypto!(ed25519, super::KEY_TYPE);
}

sp_application_crypto::with_pair! {
    pub type AuthorityPair = app::Pair;
}

pub type AuthoritySignature = app::Signature;
pub type AuthorityId = app::Public;

#[derive(PartialEq, Eq, Clone, Debug, Hash, Decode, Encode)]
pub struct Signature(pub AuthoritySignature);

impl From<AuthoritySignature> for Signature {
    fn from(authority_signature: AuthoritySignature) -> Signature {
        Signature(authority_signature)
    }
}

/// Verify the signature given an authority id.
pub fn verify(authority: &AuthorityId, message: &[u8], signature: &Signature) -> bool {
    authority.verify(&message, &signature.0)
}

/// Holds the public authority keys for a session allowing for verification of messages from that
/// session.
#[derive(PartialEq, Clone, Debug)]
pub struct AuthorityVerifier {
    authorities: Vec<AuthorityId>,
}

impl AuthorityVerifier {
    /// Constructs a new authority verifier from a set of public keys.
    pub fn new(authorities: Vec<AuthorityId>) -> Self {
        AuthorityVerifier { authorities }
    }

    /// Verifies whether the message is correctly signed with the signature assumed to be made by a
    /// node of the given index.
    pub fn verify(&self, msg: &[u8], sgn: &Signature, index: NodeIndex) -> bool {
        match self.authorities.get(index.0) {
            Some(authority) => verify(authority, msg, sgn),
            None => false,
        }
    }

    pub fn node_count(&self) -> NodeCount {
        self.authorities.len().into()
    }

    fn threshold(&self) -> usize {
        2 * self.node_count().0 / 3 + 1
    }

    /// Verifies whether the given signature set is a correct and complete multisignature of the
    /// message. Completeness requires more than 2/3 of all authorities.
    pub fn is_complete(&self, msg: &[u8], partial: &SignatureSet<Signature>) -> bool {
        let signature_count = partial.iter().count();
        if signature_count < self.threshold() {
            return false;
        }
        partial.iter().all(|(i, sgn)| self.verify(msg, sgn, i))
    }
}
