use std::{fmt::Display, hash::Hash};

use codec::Codec;

/// A public key for signature verification.
pub trait PublicKey:
    Send + Sync + Eq + Clone + AsRef<[u8]> + Display + Hash + Codec + 'static
{
    type Signature: Send + Sync + Clone + Codec;

    /// Verify whether the message has been signed with the associated private key.
    fn verify(&self, message: &[u8], signature: &Self::Signature) -> bool;
}

/// Private key for signing messages, with an associated public key.
#[async_trait::async_trait]
pub trait PrivateKey: Clone + Send + Sync + 'static {
    type Signature: Send + Sync + Clone + Codec;
    type PublicKey: PublicKey<Signature = Self::Signature>;

    /// Produce a signature for the provided message.
    async fn sign(&self, message: &[u8]) -> Self::Signature;

    /// Return the associated public key.
    fn public_key(&self) -> Self::PublicKey;
}
