use std::str::FromStr;

use aleph_client::{
    emergency_finalize, finalization_set_emergency_finalizer, BlockHash, BlockNumber,
    SignedConnection,
};
use sp_core::{ed25519, Pair};
use substrate_api_client::XtStatus;

use crate::RootConnection;

/// Sets the emergency finalized, the provided string should be the key phrase of the desired finalizer.
pub fn set_emergency_finalizer(connection: RootConnection, finalizer: String) {
    let finalizer = ed25519::Pair::from_string(&finalizer, None)
        .expect("Can parse key as ed25519")
        .public();
    finalization_set_emergency_finalizer(&connection, finalizer.into(), XtStatus::Finalized)
}

/// Finalizes the given block using the provided emergency finalizer key.
pub fn finalize(connection: SignedConnection, number: BlockNumber, hash: String, key: String) {
    let key = ed25519::Pair::from_string(&key, None).expect("Can parse key as ed25519");
    let hash = BlockHash::from_str(&hash).expect("Hash is properly hex encoded");
    emergency_finalize(&connection, number, hash, key).unwrap();
}
