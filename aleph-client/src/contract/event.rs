//! Utilities for listening for contract events.
//!
//! To use the module you will need to first create a subscription (a glorified `Receiver<String>`),
//! then run the listen loop. You might want to run the loop in a separate thread.
//!
//! ```no_run
//! # use std::sync::Arc;
//! # use std::sync::mpsc::channel;
//! # use std::thread;
//! # use std::time::Duration;
//! # use aleph_client::{Connection, SignedConnection};
//! # use aleph_client::contract::ContractInstance;
//! # use aleph_client::contract::event::{listen_contract_events, subscribe_events};
//! # use anyhow::Result;
//! # use sp_core::crypto::AccountId32;
//! # fn example(conn: SignedConnection, address1: AccountId32, address2: AccountId32, path1: &str, path2: &str) -> Result<()> {
//!     let subscription = subscribe_events(&conn)?;
//!
//!     // The `Arc` makes it possible to pass a reference to the contract to another thread
//!     let contract1 = Arc::new(ContractInstance::new(address1, path1)?);
//!     let contract2 = Arc::new(ContractInstance::new(address2, path2)?);
//!     let (cancel_tx, cancel_rx) = channel();
//!
//!     let contract1_copy = contract1.clone();
//!     let contract2_copy = contract2.clone();
//!
//!     thread::spawn(move || {
//!         listen_contract_events(
//!             subscription,
//!             &[contract1_copy.as_ref(), &contract2_copy.as_ref()],
//!             Some(cancel_rx),
//!             |event_or_error| { println!("{:?}", event_or_error) }
//!         );
//!     });
//!
//!     thread::sleep(Duration::from_secs(20));
//!     cancel_tx.send(()).unwrap();
//!
//!     contract1.contract_exec0(&conn, "some_method")?;
//!     contract2.contract_exec0(&conn, "some_other_method")?;
//!
//! #   Ok(())
//! # }
//! ```

use std::collections::HashMap;

use anyhow::{bail, Result};
use contract_transcode::Value;
use futures::{channel::mpsc::UnboundedSender, StreamExt};

use crate::{contract::ContractInstance, AccountId, Connection};

/// Represents a single event emitted by a contract.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ContractEvent {
    /// The address of the contract that emitted the event.
    pub contract: AccountId,
    /// The name of the event.
    pub ident: Option<String>,
    /// Data contained in the event.
    pub data: HashMap<String, Value>,
}

/// Starts an event listening loop.
///
/// Will execute the handler for every contract event and every error encountered while fetching
/// from `subscription`. Only events coming from the address of one of the `contracts` will be
/// decoded.
///
/// The loop will terminate once `subscription` is closed or once any message is received on
/// `cancel` (if provided).
pub async fn listen_contract_events(
    conn: &Connection,
    contracts: &[&ContractInstance],
    sender: UnboundedSender<Result<ContractEvent>>,
) -> Result<()> {
    let mut block_subscription = conn.client.blocks().subscribe_finalized().await?;

    while let Some(block) = block_subscription.next().await {
        if sender.is_closed() {
            break;
        }

        let block = block?;

        for event in block.events().await?.iter() {
            let event = event?;

            if let Some(event) =
                event.as_event::<crate::api::contracts::events::ContractEmitted>()?
            {
                if let Some(contract) = contracts
                    .iter()
                    .find(|contract| contract.address() == &event.contract)
                {
                    let data = zero_prefixed(&event.data);
                    let event = contract
                        .transcoder
                        .decode_contract_event(&mut data.as_slice());

                    sender.unbounded_send(build_event(contract.address().clone(), event))?;
                }
            }
        }
    }

    Ok(())
}

/// The contract transcoder assumes there is an extra byte (that it discards) indicating the size of the data. However,
/// data arriving through the subscription as used in this file don't have this extra byte. This function adds it.
fn zero_prefixed(data: &[u8]) -> Vec<u8> {
    let mut result = vec![0];
    result.extend_from_slice(data);
    result
}

fn build_event(address: AccountId, event_data: Result<Value>) -> Result<ContractEvent> {
    let event_data = event_data?;

    match event_data {
        Value::Map(map) => Ok(ContractEvent {
            contract: address,
            ident: map.ident(),
            data: map
                .iter()
                .map(|(key, value)| (key.to_string(), value.clone()))
                .collect(),
        }),
        _ => bail!("Contract event data is not a map"),
    }
}
