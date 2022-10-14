use primitives::{SessionIndex, Version};
use sp_core::Pair;
use substrate_api_client::{compose_call, compose_extrinsic, AccountId, ExtrinsicParams, XtStatus};

use crate::{next_session_finality_version, send_xt, AnyConnection, ReadStorage, RootConnection};

const PALLET: &str = "Aleph";
const EMERGENCY_FINALIZER: &str = "set_emergency_finalizer";
const VERSION_CHANGE: &str = "schedule_finality_version_change";

/// Sets the emergency finalizer to the provided `AccountId`.
pub fn set_emergency_finalizer(
    connection: &RootConnection,
    finalizer: AccountId,
    status: XtStatus,
) {
    let set_emergency_finalizer_call = compose_call!(
        connection.as_connection().metadata,
        PALLET,
        EMERGENCY_FINALIZER,
        finalizer
    );
    let xt = compose_extrinsic!(
        connection.as_connection(),
        "Sudo",
        "sudo_unchecked_weight",
        set_emergency_finalizer_call,
        0_u64
    );
    send_xt(connection, xt, Some(EMERGENCY_FINALIZER), status);
}

pub fn schedule_finality_version_change(
    connection: &RootConnection,
    version_incoming: Version,
    session: SessionIndex,
    status: XtStatus,
) {
    let schedule_next_finality_version_change_call = compose_call!(
        connection.as_connection().metadata,
        PALLET,
        VERSION_CHANGE,
        version_incoming,
        session
    );

    let xt = compose_extrinsic!(
        connection.as_connection(),
        "Sudo",
        "sudo_unchecked_weight",
        schedule_next_finality_version_change_call,
        0_u64
    );

    send_xt(connection, xt, Some(VERSION_CHANGE), status);
}

pub fn get_current_finality_version<C: ReadStorage>(connection: &C) -> Version {
    connection.read_storage_value(PALLET, "FinalityVersion")
}

pub fn get_next_session_finality_version<C: AnyConnection>(connection: &C) -> Version {
    connection
        .as_connection()
        .get_request(next_session_finality_version())
        .expect("Call to get next session finality version has failed!")
        .expect("Could not obtain the finality version for the next session from the runtime!")
        .parse::<Version>()
        .expect("Invalid finality version format!")
}
