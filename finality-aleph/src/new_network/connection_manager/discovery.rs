use crate::{
    new_network::{
        connection_manager::{Authentication, Multiaddr, SessionHandler},
        DataCommand, PeerId, Protocol,
    },
    NodeIndex, SessionId,
};
use codec::{Decode, Encode};
use std::{
    collections::{HashMap, HashSet},
    time,
};

/// Messages used for discovery and authentication.
#[derive(Clone, Debug, Encode, Decode)]
pub enum DiscoveryMessage {
    // Contains the id of the broadcast, to avoid spam.
    AuthenticationBroadcast(Authentication),
    // Requests always contain own authentication, to avoid asymetric trust.
    Request(Vec<NodeIndex>, Authentication),
    Authentications(Vec<Authentication>),
}

impl DiscoveryMessage {
    pub fn session_id(&self) -> SessionId {
        use DiscoveryMessage::*;
        match self {
            AuthenticationBroadcast((auth_data, _)) => auth_data.session(),
            Request(_, (auth_data, _)) => auth_data.session(),
            Authentications(auths) => match auths.get(0) {
                Some((auth_data, _)) => auth_data.session(),
                None => SessionId(0), // Broken message anyway, value doesn't matter.
            },
        }
    }
}

/// Handles creating and responding to discovery messages.
pub struct Discovery {
    cooldown: time::Duration,
    last_broadcast: HashMap<NodeIndex, time::Instant>,
    last_response: HashMap<NodeIndex, time::Instant>,
    requested_authentications: HashMap<NodeIndex, HashSet<NodeIndex>>,
    next_query: NodeIndex,
}

fn authentication_broadcast(authentication: Authentication) -> (DiscoveryMessage, DataCommand) {
    (
        DiscoveryMessage::AuthenticationBroadcast(authentication),
        DataCommand::Broadcast,
    )
}

fn request(
    missing_authorities: Vec<NodeIndex>,
    authentication: Authentication,
    peer_id: PeerId,
) -> (DiscoveryMessage, DataCommand) {
    (
        DiscoveryMessage::Request(missing_authorities, authentication),
        DataCommand::SendTo(peer_id, Protocol::Generic),
    )
}

fn response(
    authentications: Vec<Authentication>,
    peer_id: PeerId,
) -> (DiscoveryMessage, DataCommand) {
    (
        DiscoveryMessage::Authentications(authentications),
        DataCommand::SendTo(peer_id, Protocol::Generic),
    )
}

impl Discovery {
    /// Create a new discovery handler with the given response/broadcast cooldown.
    pub fn new(cooldown: time::Duration) -> Self {
        Discovery {
            cooldown,
            last_broadcast: HashMap::new(),
            last_response: HashMap::new(),
            requested_authentications: HashMap::new(),
            next_query: NodeIndex(0),
        }
    }

    /// Returns messages that should be sent as part of authority discovery at this moment.
    pub fn discover_authorities(
        &mut self,
        handler: &SessionHandler,
    ) -> Vec<(DiscoveryMessage, DataCommand)> {
        let missing_authorities = handler.missing_nodes();
        if missing_authorities.is_empty() {
            return Vec::new();
        }
        let node_count = handler.node_count();
        let authentication = handler.authentication();
        if missing_authorities.len() * 3 > 2 * node_count.0 {
            // We know of fewer than 1/3 authorities, broadcast our authentication and hope others
            // respond in kind.
            vec![authentication_broadcast(authentication)]
        } else {
            // Attempt learning about more authorities from the ones you already know.
            let mut result = Vec::new();
            let mut target = self.next_query;
            while result.len() < 2 {
                if let Some(peer_id) = handler.peer_id(&target) {
                    result.push(request(
                        missing_authorities.clone(),
                        authentication.clone(),
                        peer_id,
                    ));
                }
                target = NodeIndex((target.0 + 1) % node_count.0);
            }
            self.next_query = target;
            result
        }
    }

    fn handle_authentication(
        &mut self,
        authentication: Authentication,
        handler: &mut SessionHandler,
    ) -> Vec<Multiaddr> {
        if !handler.handle_authentication(authentication.clone()) {
            return Vec::new();
        }
        authentication.0.address()
    }

    fn handle_broadcast(
        &mut self,
        authentication: Authentication,
        handler: &mut SessionHandler,
    ) -> (Vec<Multiaddr>, Option<(DiscoveryMessage, DataCommand)>) {
        let addresses = self.handle_authentication(authentication.clone(), handler);
        if addresses.is_empty() {
            return (addresses, None);
        }
        let node_id = authentication.0.creator();
        let rebroadcast = match self.last_broadcast.get(&node_id) {
            Some(instant) => time::Instant::now() > *instant + self.cooldown,
            None => true,
        };
        let message = match rebroadcast {
            true => {
                self.last_broadcast.insert(node_id, time::Instant::now());
                Some(authentication_broadcast(authentication))
            }
            false => None,
        };
        (addresses, message)
    }

    fn create_response(
        &mut self,
        requester_id: NodeIndex,
        node_ids: Vec<NodeIndex>,
        handler: &mut SessionHandler,
    ) -> Option<(DiscoveryMessage, DataCommand)> {
        let requested_authentications = self
            .requested_authentications
            .entry(requester_id)
            .or_default();
        requested_authentications.extend(node_ids);
        if let Some(instant) = self.last_response.get(&requester_id) {
            if time::Instant::now() < *instant + self.cooldown {
                return None;
            }
        }
        let peer_id = match handler.peer_id(&requester_id) {
            Some(peer_id) => peer_id,
            None => return None,
        };
        let authentications: Vec<_> = requested_authentications
            .iter()
            .filter_map(|id| handler.authentication_for(id))
            .collect();
        if authentications.is_empty() {
            None
        } else {
            self.last_response
                .insert(requester_id, time::Instant::now());
            self.requested_authentications.remove(&requester_id);
            Some(response(authentications, peer_id))
        }
    }

    fn handle_request(
        &mut self,
        node_ids: Vec<NodeIndex>,
        authentication: Authentication,
        handler: &mut SessionHandler,
    ) -> (Vec<Multiaddr>, Option<(DiscoveryMessage, DataCommand)>) {
        let node_id = authentication.0.creator();
        let addresses = self.handle_authentication(authentication, handler);
        if addresses.is_empty() {
            return (addresses, None);
        }
        (addresses, self.create_response(node_id, node_ids, handler))
    }

    fn handle_response(
        &mut self,
        authentications: Vec<Authentication>,
        handler: &mut SessionHandler,
    ) -> Vec<Multiaddr> {
        authentications
            .into_iter()
            .flat_map(|authentication| self.handle_authentication(authentication, handler))
            .collect()
    }

    /// Analyzes the provided message and returns all the new multiaddresses we should
    /// be connected to and any messages that we should send as a result of it.
    pub fn handle_message(
        &mut self,
        message: DiscoveryMessage,
        handler: &mut SessionHandler,
    ) -> (Vec<Multiaddr>, Option<(DiscoveryMessage, DataCommand)>) {
        use DiscoveryMessage::*;
        match message {
            AuthenticationBroadcast(authentication) => {
                self.handle_broadcast(authentication, handler)
            }
            Request(node_ids, authentication) => {
                self.handle_request(node_ids, authentication, handler)
            }
            Authentications(authentications) => {
                (self.handle_response(authentications, handler), None)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Discovery, DiscoveryMessage};
    use crate::{
        crypto::{AuthorityPen, AuthorityVerifier, KeyBox},
        new_network::{
            connection_manager::{Authentication, SessionHandler},
            DataCommand, Multiaddr, Protocol,
        },
        NodeIndex, SessionId,
    };
    use aleph_primitives::{AuthorityId, KEY_TYPE};
    use codec::Encode;
    use sc_network::{
        multiaddr::Protocol as ScProtocol, Multiaddr as ScMultiaddr, PeerId as ScPeerId,
    };
    use sp_keystore::{testing::KeyStore, CryptoStore};
    use std::{collections::HashSet, iter, net::Ipv4Addr, sync::Arc, thread::sleep, time};

    const NUM_NODES: u8 = 7;
    const MS_COOLDOWN: u64 = 200;

    fn addresses() -> Vec<Multiaddr> {
        (0..NUM_NODES)
            .map(|id| {
                ScMultiaddr::empty()
                    .with(ScProtocol::Ip4(Ipv4Addr::new(192, 168, 1, id)))
                    .with(ScProtocol::Tcp(30333))
                    .with(ScProtocol::P2p(ScPeerId::random().into()))
            })
            .collect()
    }

    async fn keyboxes() -> Vec<KeyBox> {
        let num_keyboxes: usize = NUM_NODES.into();
        let keystore = Arc::new(KeyStore::new());
        let mut auth_ids = Vec::with_capacity(num_keyboxes);
        for _ in 0..num_keyboxes {
            let pk = keystore.ed25519_generate_new(KEY_TYPE, None).await.unwrap();
            auth_ids.push(AuthorityId::from(pk));
        }
        let mut result = Vec::with_capacity(num_keyboxes);
        for i in 0..num_keyboxes {
            result.push(KeyBox::new(
                NodeIndex(i),
                AuthorityVerifier::new(auth_ids.clone()),
                AuthorityPen::new(auth_ids[i].clone(), keystore.clone())
                    .await
                    .expect("The keys should sign successfully"),
            ));
        }
        result
    }

    async fn build() -> (Discovery, Vec<SessionHandler>) {
        let mut handlers = Vec::new();
        for (keybox, address) in keyboxes().await.into_iter().zip(addresses()) {
            handlers.push(
                SessionHandler::new(keybox, SessionId(43), vec![address.into()])
                    .await
                    .unwrap(),
            );
        }
        (
            Discovery::new(time::Duration::from_millis(MS_COOLDOWN)),
            handlers,
        )
    }

    #[tokio::test]
    async fn broadcasts_when_clueless() {
        let (mut discovery, mut handlers) = build().await;
        let handler = &mut handlers[0];
        let mut messages = discovery.discover_authorities(handler);
        assert_eq!(messages.len(), 1);
        let message = messages.pop().unwrap();
        assert_eq!(message.1, DataCommand::Broadcast);
        match message.0 {
            DiscoveryMessage::AuthenticationBroadcast(authentication) => {
                assert_eq!(authentication.encode(), handler.authentication().encode())
            }
            _ => panic!("Expected an authentication broadcast, got {:?}", message.0),
        }
    }

    #[tokio::test]
    async fn requests_from_single_when_only_some_missing() {
        let num_nodes: usize = NUM_NODES.into();
        let (mut discovery, mut handlers) = build().await;
        for i in 1..num_nodes - 1 {
            let authentication = handlers[i].authentication();
            assert!(handlers[0].handle_authentication(authentication));
        }
        let handler = &mut handlers[0];
        let messages = discovery.discover_authorities(handler);
        assert_eq!(messages.len(), 2);
        for message in messages {
            assert!(matches!(message.1, DataCommand::SendTo(_, _)));
            match message.0 {
                DiscoveryMessage::Request(nodes, authentication) => {
                    assert_eq!(authentication.encode(), handler.authentication().encode());
                    assert_eq!(nodes, vec![NodeIndex(6)]);
                }
                _ => panic!("Expected a request, got {:?}", message.0),
            }
        }
    }

    #[tokio::test]
    async fn requests_nothing_when_knows_all() {
        let num_nodes: usize = NUM_NODES.into();
        let (mut discovery, mut handlers) = build().await;
        for i in 1..num_nodes {
            let authentication = handlers[i].authentication();
            assert!(handlers[0].handle_authentication(authentication));
        }
        let handler = &mut handlers[0];
        let messages = discovery.discover_authorities(handler);
        assert!(messages.is_empty());
    }

    #[tokio::test]
    async fn rebroadcasts_and_accepts_addresses() {
        let (mut discovery, mut handlers) = build().await;
        let authentication = handlers[1].authentication();
        let handler = &mut handlers[0];
        let (addresses, maybe_command) = discovery.handle_message(
            DiscoveryMessage::AuthenticationBroadcast(authentication.clone()),
            handler,
        );
        assert_eq!(addresses.len(), authentication.0.address().len());
        assert_eq!(
            addresses[0].encode(),
            authentication.0.address()[0].encode()
        );
        match maybe_command {
            Some((
                DiscoveryMessage::AuthenticationBroadcast(rebroadcast_authentication),
                DataCommand::Broadcast,
            )) => assert_eq!(authentication.encode(), rebroadcast_authentication.encode()),
            _ => panic!("Expected a rebroadcast, got {:?}", maybe_command),
        }
    }

    #[tokio::test]
    async fn does_not_rebroadcast_wrong_authentications() {
        let (mut discovery, mut handlers) = build().await;
        let (auth_data, _) = handlers[1].authentication();
        let (_, signature) = handlers[2].authentication();
        let authentication = (auth_data, signature);
        let handler = &mut handlers[0];
        let (addresses, maybe_command) = discovery.handle_message(
            DiscoveryMessage::AuthenticationBroadcast(authentication),
            handler,
        );
        assert!(addresses.is_empty());
        assert!(maybe_command.is_none());
    }

    #[tokio::test]
    async fn does_not_rebroadcast_quickly() {
        let (mut discovery, mut handlers) = build().await;
        let authentication = handlers[1].authentication();
        let handler = &mut handlers[0];
        discovery.handle_message(
            DiscoveryMessage::AuthenticationBroadcast(authentication.clone()),
            handler,
        );
        let (addresses, maybe_command) = discovery.handle_message(
            DiscoveryMessage::AuthenticationBroadcast(authentication.clone()),
            handler,
        );
        assert_eq!(addresses.len(), authentication.0.address().len());
        assert_eq!(
            addresses[0].encode(),
            authentication.0.address()[0].encode()
        );
        assert!(maybe_command.is_none());
    }

    #[tokio::test]
    async fn rebroadcasts_after_cooldown() {
        let (mut discovery, mut handlers) = build().await;
        let authentication = handlers[1].authentication();
        let handler = &mut handlers[0];
        discovery.handle_message(
            DiscoveryMessage::AuthenticationBroadcast(authentication.clone()),
            handler,
        );
        sleep(time::Duration::from_millis(MS_COOLDOWN + 5));
        let (addresses, maybe_command) = discovery.handle_message(
            DiscoveryMessage::AuthenticationBroadcast(authentication.clone()),
            handler,
        );
        assert_eq!(addresses.len(), authentication.0.address().len());
        assert_eq!(
            addresses[0].encode(),
            authentication.0.address()[0].encode()
        );
        match maybe_command {
            Some((
                DiscoveryMessage::AuthenticationBroadcast(rebroadcast_authentication),
                DataCommand::Broadcast,
            )) => assert_eq!(authentication.encode(), rebroadcast_authentication.encode()),
            _ => panic!("Expected a rebroadcast, got {:?}", maybe_command),
        }
    }

    #[tokio::test]
    async fn responds_to_correct_request_when_can() {
        let (mut discovery, mut handlers) = build().await;
        let requested_authentication = handlers[1].authentication();
        let requested_node_id = requested_authentication.0.creator();
        let requester_authentication = handlers[2].authentication();
        let handler = &mut handlers[0];
        assert!(handler.handle_authentication(requested_authentication.clone()));
        let (addresses, maybe_command) = discovery.handle_message(
            DiscoveryMessage::Request(vec![requested_node_id], requester_authentication.clone()),
            handler,
        );
        assert_eq!(addresses.len(), requester_authentication.0.address().len());
        assert_eq!(
            addresses[0].encode(),
            requester_authentication.0.address()[0].encode()
        );
        match maybe_command {
            Some((
                DiscoveryMessage::Authentications(response_authentications),
                DataCommand::SendTo(peer_id, Protocol::Generic),
            )) => {
                assert_eq!(response_authentications.len(), 1);
                let response_authentication = &response_authentications[0];
                assert_eq!(
                    requested_authentication.encode(),
                    response_authentication.encode()
                );
                assert_eq!(
                    Some(peer_id),
                    handler.peer_id(&requester_authentication.0.creator())
                )
            }
            _ => panic!("Expected a response, got {:?}", maybe_command),
        }
    }

    #[tokio::test]
    async fn does_not_respond_to_correct_request_when_cannot() {
        let (mut discovery, mut handlers) = build().await;
        let requested_node_id = NodeIndex(1);
        let requester_authentication = handlers[2].authentication();
        let handler = &mut handlers[0];
        let (addresses, maybe_command) = discovery.handle_message(
            DiscoveryMessage::Request(vec![requested_node_id], requester_authentication.clone()),
            handler,
        );
        assert_eq!(addresses.len(), requester_authentication.0.address().len());
        assert_eq!(
            addresses[0].encode(),
            requester_authentication.0.address()[0].encode()
        );
        assert!(maybe_command.is_none())
    }

    #[tokio::test]
    async fn does_not_respond_to_incorrect_request() {
        let (mut discovery, mut handlers) = build().await;
        let requested_authentication = handlers[1].authentication();
        let requested_node_id = requested_authentication.0.creator();
        let (auth_data, _) = handlers[2].authentication();
        let (_, signature) = handlers[3].authentication();
        let requester_authentication = (auth_data, signature);
        let handler = &mut handlers[0];
        let (addresses, maybe_command) = discovery.handle_message(
            DiscoveryMessage::Request(vec![requested_node_id], requester_authentication),
            handler,
        );
        assert!(addresses.is_empty());
        assert!(maybe_command.is_none());
    }

    #[tokio::test]
    async fn does_not_respond_too_quickly() {
        let (mut discovery, mut handlers) = build().await;
        let requested_authentication = handlers[1].authentication();
        let requested_node_id = requested_authentication.0.creator();
        let requester_authentication = handlers[2].authentication();
        let handler = &mut handlers[0];
        assert!(handler.handle_authentication(requested_authentication.clone()));
        let (addresses, maybe_command) = discovery.handle_message(
            DiscoveryMessage::Request(vec![requested_node_id], requester_authentication.clone()),
            handler,
        );
        assert_eq!(addresses.len(), requester_authentication.0.address().len());
        assert_eq!(
            addresses[0].encode(),
            requester_authentication.0.address()[0].encode()
        );
        match maybe_command {
            Some((
                DiscoveryMessage::Authentications(response_authentications),
                DataCommand::SendTo(peer_id, Protocol::Generic),
            )) => {
                assert_eq!(response_authentications.len(), 1);
                let response_authentication = &response_authentications[0];
                assert_eq!(
                    requested_authentication.encode(),
                    response_authentication.encode()
                );
                assert_eq!(
                    Some(peer_id),
                    handler.peer_id(&requester_authentication.0.creator())
                )
            }
            _ => panic!("Expected a response, got {:?}", maybe_command),
        }
        let (addresses, maybe_command) = discovery.handle_message(
            DiscoveryMessage::Request(vec![requested_node_id], requester_authentication.clone()),
            handler,
        );
        assert_eq!(addresses.len(), requester_authentication.0.address().len());
        assert_eq!(
            addresses[0].encode(),
            requester_authentication.0.address()[0].encode()
        );
        assert!(maybe_command.is_none());
    }

    #[tokio::test]
    async fn responds_cumulatively_after_cooldown() {
        let (mut discovery, mut handlers) = build().await;
        let requester_authentication = handlers[1].authentication();
        let available_authentications_start: usize = 2;
        let available_authentications_end: usize = (NUM_NODES - 2).into();
        let available_authentications: Vec<Authentication> = (available_authentications_start
            ..available_authentications_end)
            .map(|i| handlers[i].authentication())
            .collect();
        let handler = &mut handlers[0];
        for authentication in &available_authentications {
            assert!(handler.handle_authentication(authentication.clone()));
        }
        let requested_node_id = NodeIndex(2);
        let (addresses, maybe_command) = discovery.handle_message(
            DiscoveryMessage::Request(vec![requested_node_id], requester_authentication.clone()),
            handler,
        );
        assert_eq!(addresses.len(), requester_authentication.0.address().len());
        assert_eq!(
            addresses[0].encode(),
            requester_authentication.0.address()[0].encode()
        );
        match maybe_command {
            Some((
                DiscoveryMessage::Authentications(response_authentications),
                DataCommand::SendTo(peer_id, Protocol::Generic),
            )) => {
                assert_eq!(response_authentications.len(), 1);
                let response_authentication = &response_authentications[0];
                assert_eq!(
                    available_authentications[0].encode(),
                    response_authentication.encode()
                );
                assert_eq!(
                    Some(peer_id),
                    handler.peer_id(&requester_authentication.0.creator())
                )
            }
            _ => panic!("Expected a response, got {:?}", maybe_command),
        }
        let requested_node_id = NodeIndex(3);
        let (_, maybe_command) = discovery.handle_message(
            DiscoveryMessage::Request(vec![requested_node_id], requester_authentication.clone()),
            handler,
        );
        assert!(maybe_command.is_none());
        sleep(time::Duration::from_millis(MS_COOLDOWN + 5));
        let requested_node_id = NodeIndex(available_authentications_end);
        let (_, maybe_command) = discovery.handle_message(
            DiscoveryMessage::Request(vec![requested_node_id], requester_authentication.clone()),
            handler,
        );
        match maybe_command {
            Some((
                DiscoveryMessage::Authentications(response_authentications),
                DataCommand::SendTo(peer_id, Protocol::Generic),
            )) => {
                assert_eq!(response_authentications.len(), 1);
                let response_authentication = &response_authentications[0];
                assert_eq!(
                    available_authentications[1].encode(),
                    response_authentication.encode()
                );
                assert_eq!(
                    Some(peer_id),
                    handler.peer_id(&requester_authentication.0.creator())
                )
            }
            _ => panic!("Expected a response, got {:?}", maybe_command),
        }
    }

    #[tokio::test]
    async fn accepts_correct_authentications() {
        let (mut discovery, mut handlers) = build().await;
        let authentications_start: usize = 1;
        let authentications_end: usize = (NUM_NODES - 2).into();
        let authentications =
            (authentications_start..authentications_end).map(|i| handlers[i].authentication());
        let expected_addresses: HashSet<_> = authentications
            .clone()
            .flat_map(|(auth_data, _)| auth_data.address())
            .map(|address| address.encode())
            .collect();
        let authentications = authentications.collect();
        let handler = &mut handlers[0];
        let (addresses, maybe_command) =
            discovery.handle_message(DiscoveryMessage::Authentications(authentications), handler);
        let addresses: HashSet<_> = addresses
            .into_iter()
            .map(|address| address.encode())
            .collect();
        assert_eq!(addresses, expected_addresses);
        assert!(maybe_command.is_none());
    }

    #[tokio::test]
    async fn does_not_accept_incorrect_authentications() {
        let (mut discovery, mut handlers) = build().await;
        let authentications_start: usize = 1;
        let authentications_end: usize = (NUM_NODES - 2).into();
        let authentications =
            (authentications_start..authentications_end).map(|i| handlers[i].authentication());
        let (auth_data, _) = handlers[authentications_end].authentication();
        let (_, signature) = handlers[authentications_end - 1].authentication();
        let incorrect_authentication = (auth_data, signature);
        let expected_addresses: HashSet<_> = authentications
            .clone()
            .flat_map(|(auth_data, _)| auth_data.address())
            .map(|address| address.encode())
            .collect();
        let authentications = iter::once(incorrect_authentication)
            .chain(authentications)
            .collect();
        let handler = &mut handlers[0];
        let (addresses, maybe_command) =
            discovery.handle_message(DiscoveryMessage::Authentications(authentications), handler);
        let addresses: HashSet<_> = addresses
            .into_iter()
            .map(|address| address.encode())
            .collect();
        assert_eq!(addresses, expected_addresses);
        assert!(maybe_command.is_none());
    }
}
