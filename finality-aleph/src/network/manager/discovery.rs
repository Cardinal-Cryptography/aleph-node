use std::{
    collections::HashMap,
    marker::PhantomData,
    time::{Duration, Instant},
};

use codec::{Decode, Encode};
use log::{debug, info, trace};

use crate::{
    network::{
        manager::{Authentication, SessionHandler},
        Multiaddress,
    },
    NodeIndex, SessionId,
};

/// Messages used for discovery and authentication.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum DiscoveryMessage<M: Multiaddress> {
    AuthenticationBroadcast(Authentication<M>),
    Authentication(Authentication<M>),
}

impl<M: Multiaddress> DiscoveryMessage<M> {
    pub fn session_id(&self) -> SessionId {
        use DiscoveryMessage::*;
        match self {
            AuthenticationBroadcast((auth_data, _)) | Authentication((auth_data, _)) => {
                auth_data.session()
            }
        }
    }
}

/// Handles creating and rebroadcasting discovery messages.
pub struct Discovery<M: Multiaddress> {
    cooldown: Duration,
    last_broadcast: HashMap<NodeIndex, Instant>,
    _phantom: PhantomData<M>,
}

impl<M: Multiaddress> Discovery<M> {
    /// Create a new discovery handler with the given response/broadcast cooldown.
    pub fn new(cooldown: Duration) -> Self {
        Discovery {
            cooldown,
            last_broadcast: HashMap::new(),
            _phantom: PhantomData,
        }
    }

    /// Returns a message that should be sent as part of authority discovery at this moment.
    pub fn discover_authorities(
        &mut self,
        handler: &SessionHandler<M>,
    ) -> Option<DiscoveryMessage<M>> {
        let authentication = match handler.authentication() {
            Some(authentication) => authentication,
            None => return None,
        };

        let missing_authorities = handler.missing_nodes();
        let node_count = handler.node_count();
        info!(target: "aleph-network", "{}/{} authorities known for session {}.", node_count.0-missing_authorities.len(), node_count.0, handler.session_id().0);
        Some(DiscoveryMessage::AuthenticationBroadcast(authentication))
    }

    /// Checks the authentication using the handler and returns the addresses we should be
    /// connected to if the authentication is correct.
    fn handle_authentication(
        &mut self,
        authentication: Authentication<M>,
        handler: &mut SessionHandler<M>,
    ) -> Vec<M> {
        if !handler.handle_authentication(authentication.clone()) {
            return Vec::new();
        }
        authentication.0.addresses()
    }

    fn should_rebroadcast(&self, node_id: &NodeIndex) -> bool {
        match self.last_broadcast.get(node_id) {
            Some(instant) => Instant::now() > *instant + self.cooldown,
            None => true,
        }
    }

    fn handle_broadcast(
        &mut self,
        authentication: Authentication<M>,
        handler: &mut SessionHandler<M>,
    ) -> (Vec<M>, Option<DiscoveryMessage<M>>) {
        debug!(target: "aleph-network", "Handling broadcast with authentication {:?}.", authentication);
        let addresses = self.handle_authentication(authentication.clone(), handler);
        if addresses.is_empty() {
            return (Vec::new(), None);
        }
        let node_id = authentication.0.creator();
        if !self.should_rebroadcast(&node_id) {
            return (addresses, None);
        }
        trace!(target: "aleph-network", "Rebroadcasting {:?}.", authentication);
        self.last_broadcast.insert(node_id, Instant::now());
        (
            addresses,
            Some(DiscoveryMessage::AuthenticationBroadcast(authentication)),
        )
    }

    /// Analyzes the provided message and returns all the new multiaddresses we should
    /// be connected to if we want to stay connected to the committee and an optional
    /// message that we should send as a result of it.
    pub fn handle_message(
        &mut self,
        message: DiscoveryMessage<M>,
        handler: &mut SessionHandler<M>,
    ) -> (Vec<M>, Option<DiscoveryMessage<M>>) {
        use DiscoveryMessage::*;
        match message {
            AuthenticationBroadcast(authentication) => {
                self.handle_broadcast(authentication, handler)
            }
            Authentication(authentication) => {
                (self.handle_authentication(authentication, handler), None)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{thread::sleep, time::Duration};

    use codec::Encode;

    use super::{Discovery, DiscoveryMessage};
    use crate::{
        network::{
            manager::SessionHandler,
            mock::{crypto_basics, MockMultiaddress, MockPeerId},
        },
        SessionId,
    };

    const NUM_NODES: u8 = 7;
    const MS_COOLDOWN: u64 = 200;

    fn addresses() -> Vec<MockMultiaddress> {
        (0..NUM_NODES)
            .map(|_| MockMultiaddress::random_with_id(MockPeerId::random()))
            .collect()
    }

    async fn build_number(
        num_nodes: u8,
    ) -> (
        Discovery<MockMultiaddress>,
        Vec<SessionHandler<MockMultiaddress>>,
        SessionHandler<MockMultiaddress>,
    ) {
        let crypto_basics = crypto_basics(num_nodes.into()).await;
        let mut handlers = Vec::new();
        for (authority_index_and_pen, address) in crypto_basics.0.into_iter().zip(addresses()) {
            handlers.push(
                SessionHandler::new(
                    Some(authority_index_and_pen),
                    crypto_basics.1.clone(),
                    SessionId(43),
                    vec![address],
                )
                .await
                .unwrap(),
            );
        }
        let non_validator = SessionHandler::new(
            None,
            crypto_basics.1.clone(),
            SessionId(43),
            vec![MockMultiaddress::random_with_id(MockPeerId::random())],
        )
        .await
        .unwrap();
        (
            Discovery::new(Duration::from_millis(MS_COOLDOWN)),
            handlers,
            non_validator,
        )
    }

    async fn build() -> (
        Discovery<MockMultiaddress>,
        Vec<SessionHandler<MockMultiaddress>>,
        SessionHandler<MockMultiaddress>,
    ) {
        build_number(NUM_NODES).await
    }

    #[tokio::test]
    async fn broadcasts_when_clueless() {
        for num_nodes in 2..NUM_NODES {
            let (mut discovery, mut handlers, _) = build_number(num_nodes).await;
            let handler = &mut handlers[0];
            let message = discovery.discover_authorities(handler);
            assert_eq!(
                message.expect("there is a discovery message"),
                DiscoveryMessage::AuthenticationBroadcast(handler.authentication().unwrap()),
            );
        }
    }

    #[tokio::test]
    async fn non_validator_discover_authorities_returns_empty_vector() {
        let (mut discovery, _, non_validator) = build().await;
        let message = discovery.discover_authorities(&non_validator);
        assert!(message.is_none());
    }

    #[tokio::test]
    async fn rebroadcasts_and_accepts_addresses() {
        let (mut discovery, mut handlers, _) = build().await;
        let authentication = handlers[1].authentication().unwrap();
        let handler = &mut handlers[0];
        let (addresses, command) = discovery.handle_message(
            DiscoveryMessage::AuthenticationBroadcast(authentication.clone()),
            handler,
        );
        assert_eq!(addresses, authentication.0.addresses());
        assert!(matches!(command, Some(
                DiscoveryMessage::AuthenticationBroadcast(rebroadcast_authentication),
            ) if rebroadcast_authentication == authentication));
    }

    #[tokio::test]
    async fn non_validators_rebroadcasts() {
        let (mut discovery, handlers, mut non_validator) = build().await;
        let authentication = handlers[1].authentication().unwrap();
        let (addresses, command) = discovery.handle_message(
            DiscoveryMessage::AuthenticationBroadcast(authentication.clone()),
            &mut non_validator,
        );
        assert_eq!(addresses, authentication.0.addresses());
        assert!(matches!(command, Some(
                DiscoveryMessage::AuthenticationBroadcast(rebroadcast_authentication),
            ) if rebroadcast_authentication == authentication));
    }

    #[tokio::test]
    async fn does_not_rebroadcast_wrong_authentications() {
        let (mut discovery, mut handlers, _) = build().await;
        let (auth_data, _) = handlers[1].authentication().unwrap();
        let (_, signature) = handlers[2].authentication().unwrap();
        let authentication = (auth_data, signature);
        let handler = &mut handlers[0];
        let (addresses, command) = discovery.handle_message(
            DiscoveryMessage::AuthenticationBroadcast(authentication),
            handler,
        );
        assert!(addresses.is_empty());
        assert!(command.is_none());
    }

    #[tokio::test]
    async fn rebroadcasts_after_cooldown() {
        let (mut discovery, mut handlers, _) = build().await;
        let authentication = handlers[1].authentication().unwrap();
        let handler = &mut handlers[0];
        discovery.handle_message(
            DiscoveryMessage::AuthenticationBroadcast(authentication.clone()),
            handler,
        );
        sleep(Duration::from_millis(MS_COOLDOWN + 5));
        let (addresses, command) = discovery.handle_message(
            DiscoveryMessage::AuthenticationBroadcast(authentication.clone()),
            handler,
        );
        assert_eq!(addresses, authentication.0.addresses());
        assert!(matches!(command, Some(
                DiscoveryMessage::AuthenticationBroadcast(rebroadcast_authentication),
            ) if rebroadcast_authentication == authentication));
    }

    #[tokio::test]
    async fn accepts_correct_authentications() {
        let (mut discovery, mut handlers, _) = build().await;
        let expected_address = handlers[1].authentication().unwrap().0.addresses()[0].encode();
        let authentication = handlers[1].authentication().unwrap();
        let handler = &mut handlers[0];
        let (addresses, command) =
            discovery.handle_message(DiscoveryMessage::Authentication(authentication), handler);
        assert_eq!(addresses.len(), 1);
        let address = addresses[0].encode();
        assert_eq!(address, expected_address);
        assert!(command.is_none());
    }

    #[tokio::test]
    async fn does_not_accept_incorrect_authentications() {
        let (mut discovery, mut handlers, _) = build().await;
        let (auth_data, _) = handlers[1].authentication().unwrap();
        let (_, signature) = handlers[2].authentication().unwrap();
        let incorrect_authentication = (auth_data, signature);
        let handler = &mut handlers[0];
        let (addresses, command) = discovery.handle_message(
            DiscoveryMessage::Authentication(incorrect_authentication),
            handler,
        );
        assert!(addresses.is_empty());
        assert!(command.is_none());
    }
}
