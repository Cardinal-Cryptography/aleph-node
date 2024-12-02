use std::{
    fmt::{Display, Error as FmtError, Formatter},
    sync::Arc,
};

use parity_scale_codec::{Decode, Encode};
use sc_keystore::{Keystore, LocalKeystore};
use sp_core::crypto::KeyTypeId;
use sp_keystore::Error as KeystoreError;

use crate::aleph_primitives::{AuthorityId, AuthoritySignature, KEY_TYPE};

pub use crate::aleph_primitives::crypto::{AuthorityVerifier, Signature};
pub use crate::aleph_primitives::NodeIndex;

#[derive(Debug)]
pub enum Error {
    KeyMissing(AuthorityId),
    Keystore(KeystoreError),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), FmtError> {
        use Error::*;
        match self {
            KeyMissing(auth_id) => {
                write!(f, "no key for authority {auth_id:?}")
            }
            Keystore(err) => {
                write!(f, "keystore error: {err}")
            }
        }
    }
}

/// Ties an authority identification and a cryptography keystore together for use in
/// signing that requires an authority.
#[derive(Clone)]
pub struct AuthorityPen {
    key_type_id: KeyTypeId,
    authority_id: AuthorityId,
    keystore: Arc<LocalKeystore>,
}

impl AuthorityPen {
    /// Constructs a new authority cryptography keystore for the given ID and key type.
    /// Will attempt to sign a test message to verify that signing works.
    /// Returns errors if anything goes wrong during this attempt, otherwise we assume the
    /// AuthorityPen will work for any future attempts at signing.
    pub fn new_with_key_type(
        authority_id: AuthorityId,
        keystore: Arc<LocalKeystore>,
        key_type: KeyTypeId,
    ) -> Result<Self, Error> {
        // Check whether this signing setup works
        let _: AuthoritySignature = keystore
            .ed25519_sign(key_type, &authority_id.clone().into(), b"test")
            .map_err(Error::Keystore)?
            .ok_or_else(|| Error::KeyMissing(authority_id.clone()))?
            .into();
        Ok(AuthorityPen {
            key_type_id: key_type,
            authority_id,
            keystore,
        })
    }

    /// Constructs a new authority cryptography keystore for the given ID and the aleph key type.
    /// Will attempt to sign a test message to verify that signing works.
    /// Returns errors if anything goes wrong during this attempt, otherwise we assume the
    /// AuthorityPen will work for any future attempts at signing.
    pub fn new(authority_id: AuthorityId, keystore: Arc<LocalKeystore>) -> Result<Self, Error> {
        Self::new_with_key_type(authority_id, keystore, KEY_TYPE)
    }

    /// Cryptographically signs the message.
    pub fn sign(&self, msg: &[u8]) -> Signature {
        Signature(
            self.keystore
                .ed25519_sign(self.key_type_id, &self.authority_id.clone().into(), msg)
                .expect("the keystore works")
                .expect("we have the required key")
                .into(),
        )
    }

    /// Return the associated AuthorityId.
    pub fn authority_id(&self) -> AuthorityId {
        self.authority_id.clone()
    }
}
/// Old format of signatures, needed for backwards compatibility.
#[derive(PartialEq, Eq, Clone, Debug, Decode, Encode)]
pub struct SignatureV1 {
    pub _id: NodeIndex,
    pub sgn: AuthoritySignature,
}

impl From<SignatureV1> for Signature {
    fn from(sig_v1: SignatureV1) -> Signature {
        Signature(sig_v1.sgn)
    }
}

#[cfg(test)]
mod tests {
    use sp_keystore::Keystore as _;

    use super::*;
    use crate::abft::NodeIndex;

    fn generate_keys(names: &[String]) -> (Vec<AuthorityPen>, AuthorityVerifier) {
        let key_store = Arc::new(LocalKeystore::in_memory());
        let mut authority_ids = Vec::with_capacity(names.len());
        for name in names {
            let pk = key_store
                .ed25519_generate_new(KEY_TYPE, Some(name))
                .unwrap();
            authority_ids.push(AuthorityId::from(pk));
        }
        let mut pens = Vec::with_capacity(names.len());
        for authority_id in authority_ids.clone() {
            pens.push(
                AuthorityPen::new(authority_id, key_store.clone())
                    .expect("The keys should sign successfully"),
            );
        }
        assert_eq!(key_store.keys(KEY_TYPE).unwrap().len(), names.len());
        (pens, AuthorityVerifier::new(authority_ids))
    }

    fn prepare_test() -> (Vec<AuthorityPen>, AuthorityVerifier) {
        let authority_names: Vec<_> = ["//Alice", "//Bob", "//Charlie"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        generate_keys(&authority_names)
    }

    #[test]
    fn produces_and_verifies_correct_signatures() {
        let (pens, verifier) = prepare_test();
        let msg = b"test";
        for (i, pen) in pens.into_iter().enumerate() {
            let signature = pen.sign(msg);
            assert!(verifier.verify(msg, &signature, NodeIndex(i)));
        }
    }

    #[test]
    fn does_not_accept_signatures_from_wrong_sources() {
        let (pens, verifier) = prepare_test();
        let msg = b"test";
        for pen in &pens[1..] {
            let signature = pen.sign(msg);
            assert!(!verifier.verify(msg, &signature, NodeIndex(0)));
        }
    }

    #[test]
    fn does_not_accept_signatures_from_unknown_sources() {
        let (pens, verifier) = prepare_test();
        let msg = b"test";
        for pen in &pens {
            let signature = pen.sign(msg);
            assert!(!verifier.verify(msg, &signature, NodeIndex(pens.len())));
        }
    }

    #[test]
    fn does_not_accept_signatures_for_different_messages() {
        let (pens, verifier) = prepare_test();
        let msg = b"test";
        let not_msg = b"not test";
        for (i, pen) in pens.into_iter().enumerate() {
            let signature = pen.sign(msg);
            assert!(!verifier.verify(not_msg, &signature, NodeIndex(i)));
        }
    }
}
