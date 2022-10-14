use std::{
    collections::HashMap,
    sync::mpsc::{channel, Receiver},
};

use ac_node_api::events::{EventsDecoder, Raw, RawEvent};
use anyhow::{bail, Context, Result};
use contract_transcode::{ContractMessageTranscoder, Transcoder, TranscoderBuilder, Value};
use ink_metadata::InkProject;
use sp_core::crypto::{AccountId32, Ss58Codec};
use substrate_api_client::Metadata;

use crate::{contract::ContractInstance, AnyConnection};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ContractEvent {
    contract: AccountId32,
    ident: Option<String>,
    data: HashMap<String, Value>,
}

/// An opaque wrapper around a `Receiver<String>` that can be used to listen for contract events.
pub struct EventSubscription(Receiver<String>);

/// Creates a subscription to all events that can be used to `listen_for_contract_events`
pub fn subscribe_events<C: AnyConnection>(conn: &C) -> Result<EventSubscription> {
    let (tx, rx) = channel();

    conn.as_connection().subscribe_events(tx)?;

    Ok(EventSubscription(rx))
}

/// Starts an event listening loop.
///
/// Will execute the handler for every contract event and every error encountered while fetching
/// from `subscription`. The loop will terminate `subscription` is closed. Only events coming from
/// one of the `contracts` will be decoded.
pub fn listen_contract_events<F: Fn(Result<ContractEvent>)>(
    subscription: EventSubscription,
    metadata: &Metadata,
    contracts: &[&ContractInstance],
    handler: F,
) {
    let events_decoder = EventsDecoder::new(metadata.clone());
    let events_transcoder = TranscoderBuilder::new(&metadata.runtime_metadata().types)
        .with_default_custom_type_transcoders()
        .done();
    let contracts = contracts
        .iter()
        .map(|contract| (contract.address().clone(), contract.ink_project()))
        .collect::<HashMap<_, _>>();

    for batch in subscription.0.iter() {
        match decode_contract_event_batch(
            metadata,
            &events_decoder,
            &events_transcoder,
            &contracts,
            batch,
        ) {
            Ok(events) => {
                for event in events {
                    handler(event);
                }
            }
            Err(err) => handler(Err(err)),
        }
    }
}

/// Consumes a raw `batch` of chain events, and returns only those that are coming from `contracts`.
fn decode_contract_event_batch(
    metadata: &Metadata,
    events_decoder: &EventsDecoder,
    events_transcoder: &Transcoder,
    contracts: &HashMap<AccountId32, &InkProject>,
    batch: String,
) -> Result<Vec<Result<ContractEvent>>> {
    let mut results = vec![];

    let batch = batch.replacen("0x", "", 1);
    let bytes = hex::decode(batch)?;
    let events = events_decoder.decode_events(&mut bytes.as_slice())?;

    for (_phase, raw_event) in events {
        match raw_event {
            Raw::Error(err) => results.push(Err(err.into())),
            Raw::Event(event) => {
                if event.pallet == "Contracts" && event.variant == "ContractEmitted" {
                    results.push(decode_contract_event(
                        metadata,
                        contracts,
                        events_transcoder,
                        event,
                    ))
                }
            }
        }
    }

    Ok(results)
}

fn decode_contract_event(
    metadata: &Metadata,
    contracts: &HashMap<AccountId32, &InkProject>,
    events_transcoder: &Transcoder,
    event: RawEvent,
) -> Result<ContractEvent> {
    let event_metadata = metadata.event(event.pallet_index, event.variant_index)?;

    let parse_pointer = &mut event.data.0.as_slice();
    let mut raw_data = None;
    let mut contract_address = None;

    for field in event_metadata.variant().fields() {
        if field.name() == Some(&"data".to_string()) {
            raw_data = Some(<&[u8]>::clone(parse_pointer));
        } else {
            let field_value = events_transcoder.decode(field.ty().id(), parse_pointer);

            if field.name() == Some(&"contract".to_string()) {
                contract_address = field_value.ok();
            }
        }
    }

    if let Some(Value::Literal(address)) = contract_address {
        let address = AccountId32::from_string(&address)?;
        let contract_metadata = contracts
            .get(&address)
            .context("Event from unknown contract")?;

        let mut raw_data = raw_data.context("Event data field not found")?;
        let event_data = ContractMessageTranscoder::new(contract_metadata)
            .decode_contract_event(&mut raw_data)
            .context("Failed to decode contract event")?;

        build_event(address, event_data)
    } else {
        bail!("Contract event did not contain contract address");
    }
}

fn build_event(address: AccountId32, event_data: Value) -> Result<ContractEvent> {
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
