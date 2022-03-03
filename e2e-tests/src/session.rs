use crate::AccountId;
use codec::Decode;
use common::{SessionKeys, wait_for_event, send_xt, BlockNumber, Connection, KeyPair, create_connection};
use log::info;
use sp_core::Pair;
use substrate_api_client::{compose_call, compose_extrinsic, XtStatus};

pub fn send_change_members(sudo_connection: &Connection, new_members: Vec<AccountId>) {
    info!("New members {:#?}", new_members);
    let call = compose_call!(
        sudo_connection.metadata,
        "Elections",
        "change_members",
        new_members
    );
    let xt = compose_extrinsic!(
        sudo_connection,
        "Sudo",
        "sudo_unchecked_weight",
        call,
        0_u64
    );
    send_xt(
        &sudo_connection,
        xt.hex_encode(),
        "sudo_unchecked_weight",
        XtStatus::InBlock,
    );
}

pub fn session_set_keys(
    address: &str,
    signer: &KeyPair,
    new_keys: SessionKeys,
    tx_status: XtStatus,
) {
    let connection = create_connection(address).set_signer(signer.clone());
    let xt = compose_extrinsic!(connection, "Session", "set_keys", new_keys, 0u8);
    send_xt(&connection, xt.hex_encode(), "set_keys", tx_status);
}

pub fn get_current_session(connection: &Connection) -> u32 {
    connection
        .get_storage_value("Session", "CurrentIndex", None)
        .unwrap()
        .unwrap()
}

pub fn wait_for_session(
    connection: &Connection,
    session_index: u32,
) -> anyhow::Result<BlockNumber> {
    info!("Waiting for the session {}", session_index);

    #[derive(Debug, Decode, Clone)]
    struct NewSessionEvent {
        session_index: u32,
    }
    wait_for_event(
        connection,
        ("Session", "NewSession"),
        |e: NewSessionEvent| {
            info!("[+] new session {}", e.session_index);

            e.session_index == session_index
        },
    )?;
    Ok(session_index)
}
