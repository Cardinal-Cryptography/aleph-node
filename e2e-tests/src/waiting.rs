use std::sync::mpsc::channel;
use std::thread::sleep;
use std::time::Duration;

use codec::Decode;
use log::{error, info};
use substrate_api_client::ApiResult;

use crate::utils::{Connection, Header};

fn wait_for_event<E: Decode + Clone, P: Fn(E) -> bool>(
    connection: &Connection,
    event: (&str, &str),
    predicate: P,
) -> anyhow::Result<E> {
    let (module, variant) = event;
    info!("[+] Creating event subscription {}/{}", module, variant);

    let (events_in, events_out) = channel();
    connection.subscribe_events(events_in)?;

    loop {
        let args: ApiResult<E> = connection.wait_for_event(module, variant, None, &events_out);
        match args {
            Ok(event) if predicate(event.clone()) => return Ok(event),
            Ok(_) => (),
            Err(why) => error!("Error {:?}", why),
        }
    }
}

#[derive(Debug, Decode, Copy, Clone)]
struct NewSessionEvent {
    session_index: u32,
}

/// blocking wait, if ongoing session index is >= new_session_index returns the current
pub fn wait_for_session(connection: &Connection, new_session_index: u32) -> anyhow::Result<u32> {
    wait_for_event(
        connection,
        ("Session", "NewSession"),
        |e: NewSessionEvent| {
            let session_index = e.session_index;
            info!("[+] NewSession event: session index {:?}", session_index);
            session_index.ge(&new_session_index)
        },
    )
    .map(|e| e.session_index)
}

/// blocks the main thread waiting for a block with a number at least `block_number`
pub fn wait_for_finalized_block(connection: &Connection, block_number: u32) -> anyhow::Result<u32> {
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

#[derive(Debug, Decode, Copy, Clone)]
struct ProposalRejectedEvent {
    proposal_id: u32,
    _slashed: u128,
}

/// blocks the main thread waiting for a rejection for proposal with id `proposal_id`
pub fn wait_for_rejection(connection: &Connection, proposal_id: u32) -> anyhow::Result<()> {
    wait_for_event(
        connection,
        ("Treasury", "Rejected"),
        |e: ProposalRejectedEvent| {
            info!("[+] Rejected proposal {:?}", e.proposal_id);
            proposal_id.eq(&e.proposal_id)
        },
    )
    .map(|_| ())
}
