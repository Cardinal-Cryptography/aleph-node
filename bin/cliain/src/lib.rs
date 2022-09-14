mod commands;
mod contracts;
mod finalization;
mod keys;
mod runtime;
mod secret;
mod staking;
mod transfer;
mod treasury;
mod validators;
mod vesting;

use aleph_client::{
    create_connection, keypair_from_string, Connection, RootConnection, SignedConnection,
};
pub use commands::Command;
pub use contracts::{call, instantiate, instantiate_with_code, remove_code, upload_code};
pub use finalization::{finalize, set_emergency_finalizer};
pub use keys::{next_session_keys, prepare_keys, rotate_keys, set_keys};
pub use runtime::update_runtime;
pub use secret::prompt_password_hidden;
pub use staking::{bond, force_new_era, nominate, set_staking_limits, validate};
pub use transfer::transfer;
pub use treasury::{
    approve as treasury_approve, propose as treasury_propose, reject as treasury_reject,
};
pub use validators::change_validators;
pub use vesting::{vest, vest_other, vested_transfer};

pub struct ConnectionConfig {
    node_endpoint: String,
    signer_seed: String,
}

impl ConnectionConfig {
    pub fn new(node_endpoint: String, signer_seed: String) -> Self {
        ConnectionConfig {
            node_endpoint,
            signer_seed,
        }
    }
}

impl From<ConnectionConfig> for Connection {
    fn from(cfg: ConnectionConfig) -> Self {
        create_connection(cfg.node_endpoint.as_str())
    }
}

impl From<ConnectionConfig> for SignedConnection {
    fn from(cfg: ConnectionConfig) -> Self {
        let key = keypair_from_string(&cfg.signer_seed);
        SignedConnection::new(cfg.node_endpoint.as_str(), key)
    }
}

impl From<ConnectionConfig> for RootConnection {
    fn from(cfg: ConnectionConfig) -> Self {
        RootConnection::from(Into::<SignedConnection>::into(cfg))
    }
}
