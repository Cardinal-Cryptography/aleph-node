pub(crate) mod communication;
pub(crate) mod environment;
pub(crate) mod nodes;

mod key_types {
    use sp_runtime::KeyTypeId;

    pub const ALEPH: KeyTypeId = KeyTypeId(*b"alph");
}

mod app {
    use crate::key_types::ALEPH;
    use sp_application_crypto::{app_crypto, ed25519};
    app_crypto!(ed25519, ALEPH);
}

pub type AuthorityId = app::Public;

pub type AuthoritySignature = app::Signature;

// TODO: To be added from protocol, dummy for now.
pub struct Sync {}
