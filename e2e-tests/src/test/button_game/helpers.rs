use std::{
    fmt::Debug,
    sync::{
        mpsc::{channel, Receiver, RecvTimeoutError},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

use aleph_client::{
    contract::event::{listen_contract_events, subscribe_events, ContractEvent},
    AnyConnection, Balance, Connection, KeyPair, SignedConnection, XtStatus,
};
use anyhow::{bail, Result};
use itertools::Itertools;
use log::{info, warn};
use rand::Rng;
use sp_core::Pair;

use super::contracts::{ButtonInstance, PSP22TokenInstance};
use crate::{
    test::button_game::contracts::{AsContractInstance, MarketplaceInstance},
    Config,
};

/// Creates a copy of the `connection` signed by `signer`
pub(super) fn sign<C: AnyConnection>(conn: &C, signer: KeyPair) -> SignedConnection {
    SignedConnection::from_any_connection(conn, signer)
}

/// Returns a ticket token instance for the given button instance
pub(super) fn ticket_token<C: AnyConnection>(
    conn: &C,
    button: &ButtonInstance,
    config: &Config,
) -> Result<PSP22TokenInstance> {
    PSP22TokenInstance::new(
        button.ticket_token(conn)?,
        &config.test_case_params.ticket_token_metadata,
    )
}

/// Returns a reward token instance for the given button instance
pub(super) fn reward_token<C: AnyConnection>(
    conn: &C,
    button: &ButtonInstance,
    config: &Config,
) -> Result<PSP22TokenInstance> {
    PSP22TokenInstance::new(
        button.reward_token(conn)?,
        &config.test_case_params.reward_token_metadata,
    )
}

/// Returns a marketplace instance for the given button instance
pub(super) fn marketplace<C: AnyConnection>(
    conn: &C,
    button: &ButtonInstance,
    config: &Config,
) -> Result<MarketplaceInstance> {
    MarketplaceInstance::new(
        button.marketplace(conn)?,
        &config.test_case_params.marketplace_metadata,
    )
}

/// Derives a test account based on a randomized string
pub(super) fn random_account() -> KeyPair {
    aleph_client::keypair_from_string(&format!(
        "//TestAccount/{}",
        rand::thread_rng().gen::<u128>()
    ))
}

/// Transfer `amount` from `from` to `to`
pub(super) fn transfer<C: AnyConnection>(
    conn: &C,
    from: &KeyPair,
    to: &KeyPair,
    amount: Balance,
) -> () {
    aleph_client::balances_transfer(
        &SignedConnection::from_any_connection(conn, from.clone()),
        &to.public().into(),
        amount,
        XtStatus::InBlock,
    );
}

/// Returns a number representing the given amount of alephs (adding decimals)
pub(super) fn alephs(basic_unit_amount: Balance) -> Balance {
    basic_unit_amount * 1_000_000_000_000
}

pub(super) struct ButtonTestContext {
    pub button: Arc<ButtonInstance>,
    pub ticket_token: Arc<PSP22TokenInstance>,
    pub reward_token: Arc<PSP22TokenInstance>,
    pub marketplace: Arc<MarketplaceInstance>,
    pub conn: Connection,
    pub events: BufferedReceiver<Result<ContractEvent>>,
    pub authority: KeyPair,
    pub player: KeyPair,
}

pub(super) fn setup_button_test(
    config: &Config,
    button_contract_address: &Option<String>,
) -> Result<ButtonTestContext> {
    let conn = config.get_first_signed_connection().as_connection();

    let authority = aleph_client::keypair_from_string(&config.sudo_seed);
    let player = random_account();

    let button = Arc::new(ButtonInstance::new(config, button_contract_address)?);
    let ticket_token = Arc::new(ticket_token(&conn, &button, &config)?);
    let reward_token = Arc::new(reward_token(&conn, &button, &config)?);
    let marketplace = Arc::new(marketplace(&conn, &button, &config)?);

    let c1 = button.clone();
    let c2 = ticket_token.clone();
    let c3 = reward_token.clone();
    let c4 = marketplace.clone();

    let subscription = subscribe_events(&conn)?;
    let (events_tx, events_rx) = channel();

    thread::spawn(move || {
        let contract_metadata = vec![
            c1.as_contract(),
            c2.as_contract(),
            c3.as_contract(),
            c4.as_contract(),
        ];

        listen_contract_events(subscription, &contract_metadata, None, |event| {
            let _ = events_tx.send(event);
        });
    });

    let events = BufferedReceiver::new(events_rx, Duration::from_secs(3));

    transfer(&conn, &authority, &player, alephs(100));

    Ok(ButtonTestContext {
        button,
        ticket_token,
        reward_token,
        marketplace,
        conn,
        events,
        authority,
        player,
    })
}

/// A receiver where it's possible to wait for messages out of order.
pub struct BufferedReceiver<T> {
    buffer: Vec<T>,
    receiver: Receiver<T>,
    default_timeout: Duration,
}

impl<T> BufferedReceiver<T> {
    pub(super) fn new(receiver: Receiver<T>, default_timeout: Duration) -> Self {
        Self {
            buffer: Vec::new(),
            receiver,
            default_timeout,
        }
    }

    pub(super) fn recv_timeout<F: Fn(&T) -> bool>(
        &mut self,
        filter: F,
    ) -> Result<T, RecvTimeoutError> {
        match self.buffer.iter().find_position(|m| filter(m)) {
            Some((i, _)) => Ok(self.buffer.remove(i)),
            None => {
                let mut timeout = self.default_timeout;

                while timeout > Duration::from_millis(0) {
                    let start = Instant::now();
                    match self.receiver.recv_timeout(timeout) {
                        Ok(msg) => {
                            if filter(&msg) {
                                return Ok(msg);
                            } else {
                                self.buffer.push(msg);
                                timeout -= Instant::now().duration_since(start);
                            }
                        }
                        Err(_) => return Err(RecvTimeoutError::Timeout),
                    }
                }

                return Err(RecvTimeoutError::Timeout);
            }
        }
    }
}

pub(super) fn wait_for_death<C: AnyConnection>(conn: &C, button: &ButtonInstance) -> Result<()> {
    info!("Waiting for button to die");
    assert_soon(|| button.is_dead(conn), Duration::from_secs(30))
}

pub(super) fn assert_soon<F: Fn() -> Result<bool>>(check: F, timeout: Duration) -> Result<()> {
    let start = Instant::now();
    while !check()? {
        if Instant::now().duration_since(start) > timeout {
            bail!("Condition not met within timeout")
        }
    }
    Ok(())
}

pub(super) fn assert_recv_id(
    events: &mut BufferedReceiver<Result<ContractEvent>>,
    id: &str,
) -> ContractEvent {
    assert_recv(
        events,
        |event| event.ident == Some(id.to_string()),
        &format!("Expected {:?} contract event", id),
    )
}

pub(super) fn assert_recv<T: Debug, F: Fn(&T) -> bool>(
    events: &mut BufferedReceiver<Result<T>>,
    filter: F,
    context: &str,
) -> T {
    let event = recv_timeout_with_log(events, filter);

    assert!(event.is_ok(), "{}", context);

    event.unwrap()
}

pub(super) fn refute_recv_id(events: &mut BufferedReceiver<Result<ContractEvent>>, id: &str) {
    if let Ok(event) = recv_timeout_with_log(events, |event| event.ident == Some(id.to_string())) {
        assert!(false, "Received unexpected event {:?}", event);
    }
}

fn recv_timeout_with_log<T: Debug, F: Fn(&T) -> bool>(
    events: &mut BufferedReceiver<Result<T>>,
    filter: F,
) -> Result<T> {
    match events.recv_timeout(|event_or_error| {
        if event_or_error.is_ok() {
            info!("Received contract event {:?}", event_or_error);
        } else {
            warn!("Contract event error {:?}", event_or_error);
        }

        event_or_error.as_ref().map(|x| filter(x)).unwrap_or(false)
    }) {
        Ok(event) => Ok(event.unwrap()),
        Err(err) => bail!(err),
    }
}
