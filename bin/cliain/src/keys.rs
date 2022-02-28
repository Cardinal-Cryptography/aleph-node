use common::{set_keys, rotate_keys, create_connection, KeyPair};
use substrate_api_client::XtStatus;

pub fn prepare(node: String, key: KeyPair) {
    let connection = create_connection(&node).set_signer(key);

    let new_keys = rotate_keys(&connection).expect("Connection works").expect("Received new keys");
    set_keys(&connection, new_keys, XtStatus::Finalized);
}
