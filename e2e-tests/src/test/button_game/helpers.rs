use std::{
    fmt::Debug,
    ops::Deref,
    sync::{
        mpsc::{channel, Receiver, RecvTimeoutError},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

use aleph_client::{
    contract::event::{listen_contract_events, subscribe_events, ContractEvent},
    AccountId, AnyConnection, Balance, Connection, KeyPair, SignedConnection, XtStatus,
};
use anyhow::{bail, Result};
use itertools::Itertools;
use log::{info, warn};
use rand::Rng;
use sp_core::Pair;

use super::contracts::{ButtonInstance, PSP22TokenInstance};
use crate::{test::button_game::contracts::MarketplaceInstance, Config};

/// A wrapper around a KeyPair for purposes of converting to an account id in tests.
pub struct KeyPairWrapper(KeyPair);

impl Deref for KeyPairWrapper {
    type Target = KeyPair;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<&KeyPairWrapper> for AccountId {
    fn from(keypair: &KeyPairWrapper) -> Self {
        keypair.public().into()
    }
}

/// Creates a copy of the `connection` signed by `signer`
pub fn sign<C: AnyConnection>(conn: &C, signer: &KeyPair) -> SignedConnection {
    SignedConnection::from_any_connection(conn, signer.clone())
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

/// Returns a marketplace with metadata of upgraded version of the contract.
/// To be used after marketpalce's code has been upgraded.
pub(super) fn update_marketplace_metadata_to_v2(
    marketplace: Arc<MarketplaceInstance>,
    config: &Config,
) -> Arc<MarketplaceInstance> {
    Arc::from(
        MarketplaceInstance::new(
            marketplace.contract().address().clone(),
            &Some(
                config
                    .test_case_params
                    .marketplace_v2_metadata
                    .clone()
                    .expect("New code's metadata path must be specified."),
            ),
        )
        .expect("Adding existing contract should succeed."),
    )
}

/// Derives a test account based on a randomized string
pub fn random_account() -> KeyPairWrapper {
    KeyPairWrapper(aleph_client::keypair_from_string(&format!(
        "//TestAccount/{}",
        rand::thread_rng().gen::<u128>()
    )))
}

/// Transfer `amount` from `from` to `to`
pub fn transfer<C: AnyConnection>(conn: &C, from: &KeyPair, to: &KeyPair, amount: Balance) {
    aleph_client::balances_transfer(
        &SignedConnection::from_any_connection(conn, from.clone()),
        &to.public().into(),
        amount,
        XtStatus::InBlock,
    );
}

/// Returns a number representing the given amount of alephs (adding decimals)
pub fn alephs(basic_unit_amount: Balance) -> Balance {
    basic_unit_amount * 1_000_000_000_000
}

pub(super) struct ButtonTestContext {
    pub button: Arc<ButtonInstance>,
    pub ticket_token: Arc<PSP22TokenInstance>,
    pub reward_token: Arc<PSP22TokenInstance>,
    pub marketplace: Arc<MarketplaceInstance>,
    pub conn: Connection,
    /// A [BufferedReceiver] preconfigured to listen for events of `button`, `ticket_token`, `reward_token`, and
    /// `marketplace`.
    pub events: BufferedReceiver<Result<ContractEvent>>,
    /// The authority owning the initial supply of tickets and with the power to mint game tokens.
    pub authority: KeyPairWrapper,
    /// A random account with some money for transaction fees.
    pub player: KeyPairWrapper,
}

/// Sets up a number of objects commonly used in button game tests.
pub(super) fn setup_button_test(
    config: &Config,
    button_contract_address: &Option<String>,
) -> Result<ButtonTestContext> {
    let conn = config.get_first_signed_connection().as_connection();

    let authority = KeyPairWrapper(aleph_client::keypair_from_string(&config.sudo_seed));
    let player = random_account();

    let button = Arc::new(ButtonInstance::new(config, button_contract_address)?);
    let ticket_token = Arc::new(ticket_token(&conn, &button, config)?);
    let reward_token = Arc::new(reward_token(&conn, &button, config)?);
    let marketplace = Arc::new(marketplace(&conn, &button, config)?);

    let c1 = button.clone();
    let c2 = ticket_token.clone();
    let c3 = reward_token.clone();
    let c4 = marketplace.clone();

    let subscription = subscribe_events(&conn)?;
    let (events_tx, events_rx) = channel();

    thread::spawn(move || {
        let contract_metadata = vec![
            c1.as_ref().into(),
            c2.as_ref().into(),
            c3.as_ref().into(),
            c4.as_ref().into(),
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
    pub fn new(receiver: Receiver<T>, default_timeout: Duration) -> Self {
        Self {
            buffer: Vec::new(),
            receiver,
            default_timeout,
        }
    }

    /// Receive a message satisfying `filter`.
    ///
    /// If such a message was received earlier and is waiting in the buffer, returns the message immediately and removes
    /// it from the buffer. Otherwise, listens for messages for `default_timeout`, storing them in the buffer. If a
    /// matching message is found during that time, it is returned. If not, `Err(RecvTimeoutError)` is returned.
    pub fn recv_timeout<F: Fn(&T) -> bool>(&mut self, filter: F) -> Result<T, RecvTimeoutError> {
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

                Err(RecvTimeoutError::Timeout)
            }
        }
    }
}

/// Wait until `button` is dead.
///
/// Returns `Err(_)` if the button doesn't die within 30 seconds.
pub(super) fn wait_for_death<C: AnyConnection>(conn: &C, button: &ButtonInstance) -> Result<()> {
    info!("Waiting for button to die");
    assert_soon(|| button.is_dead(conn), Duration::from_secs(30))
}

/// Wait until `check` returns true.
///
/// Repeatedly performs `check` (busy wait) until `timeout` elapses. Returns `Ok(())` if `check` returns true during
/// that time, `Err(_)` otherwise.
pub fn assert_soon<F: Fn() -> Result<bool>>(check: F, timeout: Duration) -> Result<()> {
    let start = Instant::now();
    while !check()? {
        if Instant::now().duration_since(start) > timeout {
            bail!("Condition not met within timeout")
        }
    }
    Ok(())
}

/// Asserts that a message with `id` is received (within `events.default_timeout`) and returns it.
pub fn assert_recv_id(
    events: &mut BufferedReceiver<Result<ContractEvent>>,
    id: &str,
) -> ContractEvent {
    assert_recv(
        events,
        |event| event.ident == Some(id.to_string()),
        &format!("Expected {:?} contract event", id),
    )
}

/// Asserts that a message matching `filter` is received (within `events.default_timeout`) and returns it.
pub fn assert_recv<T: Debug, F: Fn(&T) -> bool>(
    events: &mut BufferedReceiver<Result<T>>,
    filter: F,
    context: &str,
) -> T {
    let event = recv_timeout_with_log(events, filter);

    assert!(event.is_ok(), "{}", context);

    event.unwrap()
}

/// Asserts that a message with `id` is not received (within `events.default_timeout`).
pub fn refute_recv_id(events: &mut BufferedReceiver<Result<ContractEvent>>, id: &str) {
    if let Ok(event) = recv_timeout_with_log(events, |event| event.ident == Some(id.to_string())) {
        panic!("Received unexpected event {:?}", event);
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

        event_or_error.as_ref().map(&filter).unwrap_or(false)
    }) {
        Ok(event) => Ok(event.unwrap()),
        Err(err) => bail!(err),
    }
}
