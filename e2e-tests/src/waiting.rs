use std::sync::mpsc::channel;
use std::thread::sleep;
use std::time::Duration;

use codec::Decode;
use log::{error, info};
use substrate_api_client::ApiResult;

use crate::utils::{Connection, Header};

#[derive(Debug, Decode)]
struct NewSessionEvent {
    session_index: u32,
}

/// blocking wait, if ongoing session index is >= new_session_index returns the current
pub fn wait_for_session(connection: Connection, new_session_index: u32) -> anyhow::Result<u32> {
    let (module, variant) = ("Session", "NewSession");
    info!("[+] Creating event subscription {}/{}", module, variant);
    let (events_in, events_out) = channel();
    connection.subscribe_events(events_in)?;

    loop {
        let args: ApiResult<NewSessionEvent> =
            connection.wait_for_event(module, variant, None, &events_out);
        match args {
            Ok(NewSessionEvent { session_index }) => {
                info!("[+] Decoded NewSession event {:?}", &session_index);
                if session_index.ge(&new_session_index) {
                    return Ok(session_index);
                }
            }
            Err(why) => error!("Error {:?}", why),
        }
    }
}

/// blocks the main thread waiting for a block with a number at least `block_number`
pub fn wait_for_finalized_block(connection: Connection, block_number: u32) -> anyhow::Result<u32> {
    let (sender, receiver) = channel();
    connection.subscribe_finalized_heads(sender)?;

    while let Ok(header) = receiver
        .recv()
        .map(|h| serde_json::from_str::<Header>(&h).unwrap())
    {
        info!("[+] Received header for a block number {:?}", header.number);

        if header.number.ge(&block_number) {
            return Ok(block_number);
        }
    }

    Err(anyhow::anyhow!("Giving up"))
}

/// blocks the main thread waiting for an approval for proposal with id `proposal_id`
pub fn wait_for_approval(connection: &Connection, proposal_id: u32) -> anyhow::Result<()> {
    loop {
        let approvals: Vec<u32> = connection
            .get_storage_value("Treasury", "Approvals", None)
            .unwrap()
            .unwrap();
        if approvals.contains(&proposal_id) {
            info!("[+] Proposal {:?} approved successfully", proposal_id);
            return Ok(());
        } else {
            info!(
                "[+] Still waiting for approval for proposal {:?}",
                proposal_id
            );
            sleep(Duration::from_millis(500))
        }
    }
}

#[derive(Decode)]
struct ProposalRejectedEvent {
    proposal_id: u32,
    _slashed: u128,
}

/// blocks the main thread waiting for a rejection for proposal with id `proposal_id`
pub fn wait_for_rejection(connection: &Connection, proposal_id: u32) -> anyhow::Result<()> {
    let (module, variant) = ("Treasury", "Rejected");
    info!("[+] Creating event subscription {}/{}", module, variant);
    let (events_in, events_out) = channel();
    connection.subscribe_events(events_in).unwrap();
    loop {
        let args: ApiResult<ProposalRejectedEvent> =
            connection.wait_for_event(module, variant, None, &events_out);
        if let Ok(event) = args {
            if proposal_id == event.proposal_id {
                info!("[+] Proposal {:?} rejected successfully", proposal_id);
                return Ok(());
            }
        }
    }
}
