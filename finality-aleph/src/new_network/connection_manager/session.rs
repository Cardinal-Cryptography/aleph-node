use crate::{
    crypto::{AuthorityPen, AuthorityVerifier},
    new_network::{
        connection_manager::{get_common_peer_id, is_p2p, AuthData, Authentication, Multiaddr},
        PeerId,
    },
    NodeIndex, SessionId,
};
use aleph_bft::NodeCount;
use codec::Encode;
use std::collections::HashMap;

/// A struct for handling authentications for a given session and maintaining
/// mappings between PeerIds and NodeIndexes within that session.
pub struct Handler {
    peers_by_node: HashMap<NodeIndex, PeerId>,
    authentications: HashMap<PeerId, (Authentication, Option<Authentication>)>,
    own_authentication: Option<Authentication>,
    own_peer_id: PeerId,
    authority_index_and_pen: Option<(NodeIndex, AuthorityPen)>,
    authority_verifier: AuthorityVerifier,
}

/// Returned when a set of addresses is not usable for creating authentications.
/// Either because none of the addresses are externally reachable libp2p addresses,
/// or the addresses contain multiple libp2p PeerIds.
#[derive(Debug)]
pub enum AddressError {
    NoP2pAddresses,
    MultiplePeerIds,
}

async fn construct_authentication(
    authority_index_and_pen: &Option<(NodeIndex, AuthorityPen)>,
    session_id: Option<SessionId>,
    addresses: Vec<Multiaddr>,
) -> Result<(Option<Authentication>, PeerId), AddressError> {
    let addresses: Vec<_> = addresses.into_iter().filter(is_p2p).collect();
    if addresses.is_empty() {
        return Err(AddressError::NoP2pAddresses);
    }
    let peer_id = match get_common_peer_id(&addresses) {
        Some(peer_id) => peer_id,
        None => return Err(AddressError::MultiplePeerIds),
    };
    if let (Some((node_index, authority_pen)), Some(unwrapped_session_id)) =
        (authority_index_and_pen, session_id)
    {
        let auth_data = AuthData {
            addresses,
            node_id: *node_index,
            session_id: unwrapped_session_id,
        };
        let signature = authority_pen.sign(&auth_data.encode()).await;
        return Ok((Some((auth_data, signature)), peer_id));
    }
    Ok((None, peer_id))
}

impl Handler {
    /// Returns an error if the set of addresses contains no external libp2p addresses, or contains
    /// at least two such addresses with differing PeerIds.
    pub async fn new(
        authority_index_and_pen: Option<(NodeIndex, AuthorityPen)>,
        authority_verifier: AuthorityVerifier,
        session_id: SessionId,
        addresses: Vec<Multiaddr>,
    ) -> Result<Handler, AddressError> {
        let (own_authentication, own_peer_id) =
            construct_authentication(&authority_index_and_pen, Some(session_id), addresses).await?;
        Ok(Handler {
            peers_by_node: HashMap::new(),
            authentications: HashMap::new(),
            own_authentication,
            authority_index_and_pen,
            authority_verifier,
            own_peer_id,
        })
    }

    fn index(&self) -> Option<NodeIndex> {
        match self.authority_index_and_pen {
            Some((index, _)) => Some(index),
            _ => None,
        }
    }

    pub fn node_count(&self) -> NodeCount {
        self.authority_verifier.node_count()
    }

    fn session_id(&self) -> Option<SessionId> {
        match self.own_authentication.as_ref() {
            Some(own_authentication) => Some(own_authentication.0.session_id),
            _ => None,
        }
    }

    /// Returns the authentication for the node and session this handler is responsible for.
    pub fn authentication(&self) -> Option<Authentication> {
        self.own_authentication.clone()
    }

    /// Returns a vector of indices of nodes for which the handler has no authentication.
    pub fn missing_nodes(&self) -> Vec<NodeIndex> {
        let node_count = self.node_count().0;
        if self.peers_by_node.len() + 1 == node_count {
            return Vec::new();
        }
        match self.index() {
            Some(index) => (0..node_count)
                .map(NodeIndex)
                .filter(|node_id| *node_id != index && !self.peers_by_node.contains_key(node_id))
                .collect(),
            _ => (0..node_count)
                .map(NodeIndex)
                .filter(|node_id| !self.peers_by_node.contains_key(node_id))
                .collect(),
        }
    }

    /// Verifies the authentication, uses it to update mappings, and returns whether we should
    /// remain connected to the multiaddresses.
    pub fn handle_authentication(&mut self, authentication: Authentication) -> bool {
        let (auth_data, signature) = authentication.clone();

        match self.session_id() {
            Some(session_id) => {
                if auth_data.session_id != session_id {
                    return false;
                }
            }
            _ => {}
        }

        // The auth is completely useless if it doesn't have a consistent PeerId.
        let peer_id = match get_common_peer_id(&auth_data.addresses) {
            Some(peer_id) => peer_id,
            None => return false,
        };
        if peer_id == self.own_peer_id {
            return false;
        }
        if !self
            .authority_verifier
            .verify(&auth_data.encode(), &signature, auth_data.node_id)
        {
            // This might be an authentication for a key that has been changed, but we are not yet
            // aware of the change.
            if let Some(auth_pair) = self.authentications.get_mut(&peer_id) {
                auth_pair.1 = Some(authentication);
            }
            return false;
        }
        self.peers_by_node.insert(auth_data.node_id, peer_id);
        self.authentications.insert(peer_id, (authentication, None));
        true
    }

    /// Returns the PeerId of the node with the given NodeIndex, if known.
    pub fn peer_id(&self, node_id: &NodeIndex) -> Option<PeerId> {
        self.peers_by_node.get(node_id).copied()
    }

    /// Returns the NodeIndex of the node with the given PeerId, if known.
    pub fn node_id(&self, peer_id: &PeerId) -> Option<NodeIndex> {
        self.authentications
            .get(peer_id)
            .map(|((auth_data, _), _)| auth_data.node_id)
    }

    /// Updates the handler with the given keychain and set of own addresses.
    /// Returns an error if the set of addresses is not valid.
    /// All authentications will be rechecked, invalid ones purged and cached ones that turn out to
    /// now be valid canonalized.
    /// Own authentication will be regenerated.
    /// If successful returns a set of addresses that we should be connected to.
    pub async fn update(
        &mut self,
        authority_index_and_pen: Option<(NodeIndex, AuthorityPen)>,
        authority_verifier: AuthorityVerifier,
        addresses: Vec<Multiaddr>,
    ) -> Result<Vec<Multiaddr>, AddressError> {
        let (own_authentication, own_peer_id) =
            construct_authentication(&authority_index_and_pen, self.session_id(), addresses)
                .await?;
        let authentications = self.authentications.clone();
        self.authentications = HashMap::new();
        self.peers_by_node = HashMap::new();
        self.authority_index_and_pen = authority_index_and_pen;
        self.authority_verifier = authority_verifier;
        self.own_authentication = own_authentication;
        self.own_peer_id = own_peer_id;
        for (_, (auth, maybe_auth)) in authentications {
            print!(
                "normal authentication: {:?}",
                self.handle_authentication(auth)
            );
            if let Some(auth) = maybe_auth {
                print!(
                    "alternative authentication: {:?}",
                    self.handle_authentication(auth)
                );
            }
        }
        Ok(self
            .authentications
            .values()
            .flat_map(|((auth_data, _), _)| auth_data.addresses.iter().cloned())
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::{get_common_peer_id, AddressError, Handler};
    use crate::{
        crypto::{AuthorityPen, AuthorityVerifier},
        new_network::connection_manager::Multiaddr,
        AuthorityId, NodeIndex, SessionId,
    };
    use aleph_primitives::KEY_TYPE;
    use sc_network::Multiaddr as ScMultiaddr;
    use sp_keystore::{testing::KeyStore, CryptoStore};
    use std::sync::Arc;

    const NUM_NODES: usize = 7;

    async fn keyboxes_components() -> Vec<(Option<(NodeIndex, AuthorityPen)>, AuthorityVerifier)> {
        let num_keyboxes_components = NUM_NODES;
        let keystore = Arc::new(KeyStore::new());
        let mut auth_ids = Vec::with_capacity(num_keyboxes_components);
        for _ in 0..num_keyboxes_components {
            let pk = keystore.ed25519_generate_new(KEY_TYPE, None).await.unwrap();
            auth_ids.push(AuthorityId::from(pk));
        }
        let mut result = Vec::with_capacity(num_keyboxes_components);
        for i in 0..num_keyboxes_components {
            result.push((
                Some((
                    NodeIndex(i),
                    AuthorityPen::new(auth_ids[i].clone(), keystore.clone())
                        .await
                        .expect("The keys should sign successfully"),
                )),
                AuthorityVerifier::new(auth_ids.clone()),
            ));
        }
        result
    }

    fn address(text: &str) -> ScMultiaddr {
        text.parse().unwrap()
    }

    fn correct_addresses_0() -> Vec<Multiaddr> {
        vec![
                address("/dns4/example.com/tcp/30333/p2p/12D3KooWRkGLz4YbVmrsWK75VjFTs8NvaBu42xhAmQaP4KeJpw1L").into(),
                address("/dns4/peer.example.com/tcp/30333/p2p/12D3KooWRkGLz4YbVmrsWK75VjFTs8NvaBu42xhAmQaP4KeJpw1L").into(),
        ]
    }

    fn correct_addresses_1() -> Vec<Multiaddr> {
        vec![
                address("/dns4/other.example.com/tcp/30333/p2p/12D3KooWFVXnvJdPuGnGYMPn5qLQAQYwmRBgo6SmEQsKZSrDoo2k").into(),
                address("/dns4/peer.other.example.com/tcp/30333/p2p/12D3KooWFVXnvJdPuGnGYMPn5qLQAQYwmRBgo6SmEQsKZSrDoo2k").into(),
        ]
    }

    fn local_p2p_addresses() -> Vec<Multiaddr> {
        vec![address(
            "/ip4/127.0.0.1/tcp/30333/p2p/12D3KooWFVXnvJdPuGnGYMPn5qLQAQYwmRBgo6SmEQsKZSrDoo2k",
        )
        .into()]
    }

    fn mixed_addresses() -> Vec<Multiaddr> {
        vec![
                address("/dns4/example.com/tcp/30333/p2p/12D3KooWRkGLz4YbVmrsWK75VjFTs8NvaBu42xhAmQaP4KeJpw1L").into(),
                address("/dns4/peer.example.com/tcp/30333/p2p/12D3KooWRkGLz4YbVmrsWK75VjFTs8NvaBu42xhAmQaP4KeJpw1L").into(),
                address("/ip4/example.com/udt/sctp/5678").into(),
                address("/ip4/81.6.39.166/udt/sctp/5678").into(),
        ]
    }

    #[tokio::test]
    async fn creates_with_correct_data() {
        let (authority_index_and_pen, authority_verifier) =
            keyboxes_components().await.pop().unwrap();
        assert!(Handler::new(
            authority_index_and_pen,
            authority_verifier,
            SessionId(43),
            correct_addresses_0()
        )
        .await
        .is_ok());
    }

    #[tokio::test]
    async fn creates_with_local_address() {
        let (authority_index_and_pen, authority_verifier) =
            keyboxes_components().await.pop().unwrap();
        assert!(Handler::new(
            authority_index_and_pen,
            authority_verifier,
            SessionId(43),
            local_p2p_addresses()
        )
        .await
        .is_ok());
    }

    #[tokio::test]
    async fn creates_without_node_index_nor_authority_pen() {
        let (_, authority_verifier) = keyboxes_components().await.pop().unwrap();
        assert!(Handler::new(
            None,
            authority_verifier,
            SessionId(43),
            correct_addresses_0()
        )
        .await
        .is_ok());
    }

    #[tokio::test]
    async fn fails_to_create_with_no_addresses() {
        let (authority_index_and_pen, authority_verifier) =
            keyboxes_components().await.pop().unwrap();
        assert!(matches!(
            Handler::new(
                authority_index_and_pen,
                authority_verifier,
                SessionId(43),
                Vec::new()
            )
            .await,
            Err(AddressError::NoP2pAddresses)
        ));
    }

    #[tokio::test]
    async fn fails_to_create_with_non_unique_peer_id() {
        let (authority_index_and_pen, authority_verifier) =
            keyboxes_components().await.pop().unwrap();
        let addresses = correct_addresses_0()
            .into_iter()
            .chain(correct_addresses_1())
            .collect();
        assert!(matches!(
            Handler::new(
                authority_index_and_pen,
                authority_verifier,
                SessionId(43),
                addresses
            )
            .await,
            Err(AddressError::MultiplePeerIds)
        ));
    }

    #[tokio::test]
    async fn misses_all_other_nodes_initially() {
        let (authority_index_and_pen, authority_verifier) =
            keyboxes_components().await.pop().unwrap();
        let handler0 = Handler::new(
            authority_index_and_pen,
            authority_verifier,
            SessionId(43),
            correct_addresses_0(),
        )
        .await
        .unwrap();
        let missing_nodes = handler0.missing_nodes();
        let expected_missing: Vec<_> = (0..NUM_NODES - 1).map(NodeIndex).collect();
        assert_eq!(missing_nodes, expected_missing);
        assert!(handler0.peer_id(&NodeIndex(1)).is_none());
    }

    #[tokio::test]
    async fn accepts_correct_authentication() {
        let keyboxes_components = keyboxes_components().await;
        let mut handler0 = Handler::new(
            keyboxes_components[0].0.clone(),
            keyboxes_components[0].1.clone(),
            SessionId(43),
            correct_addresses_0(),
        )
        .await
        .unwrap();
        let handler1 = Handler::new(
            keyboxes_components[1].0.clone(),
            keyboxes_components[1].1.clone(),
            SessionId(43),
            correct_addresses_1(),
        )
        .await
        .unwrap();
        assert!(handler0.handle_authentication(handler1.authentication().unwrap()));
        let missing_nodes = handler0.missing_nodes();
        let expected_missing: Vec<_> = (2..NUM_NODES).map(NodeIndex).collect();
        assert_eq!(missing_nodes, expected_missing);
        let peer_id1 = get_common_peer_id(&correct_addresses_1());
        assert_eq!(handler0.peer_id(&NodeIndex(1)), peer_id1);
        assert_eq!(handler0.node_id(&peer_id1.unwrap()), Some(NodeIndex(1)));
    }

    #[tokio::test]
    async fn nonvalidator_accepts_correct_authentication() {
        let keyboxes_components = keyboxes_components().await;
        let mut handler0 = Handler::new(
            None,
            keyboxes_components[0].1.clone(),
            SessionId(43),
            correct_addresses_0(),
        )
        .await
        .unwrap();
        let handler1 = Handler::new(
            keyboxes_components[1].0.clone(),
            keyboxes_components[1].1.clone(),
            SessionId(43),
            correct_addresses_1(),
        )
        .await
        .unwrap();
        assert!(handler0.handle_authentication(handler1.authentication().unwrap()));
        let missing_nodes = handler0.missing_nodes();
        let mut expected_missing: Vec<_> = (0..NUM_NODES).map(NodeIndex).collect();
        expected_missing.remove(1);
        assert_eq!(missing_nodes, expected_missing);
        let peer_id1 = get_common_peer_id(&correct_addresses_1());
        assert_eq!(handler0.peer_id(&NodeIndex(1)), peer_id1);
        assert_eq!(handler0.node_id(&peer_id1.unwrap()), Some(NodeIndex(1)));
    }

    #[tokio::test]
    async fn ignores_badly_signed_authentication() {
        let keyboxes_components = keyboxes_components().await;
        let mut handler0 = Handler::new(
            keyboxes_components[0].0.clone(),
            keyboxes_components[0].1.clone(),
            SessionId(43),
            correct_addresses_0(),
        )
        .await
        .unwrap();
        let handler1 = Handler::new(
            keyboxes_components[1].0.clone(),
            keyboxes_components[1].1.clone(),
            SessionId(43),
            correct_addresses_1(),
        )
        .await
        .unwrap();
        let mut authentication = handler1.authentication().unwrap();
        authentication.1 = handler0.authentication().unwrap().1;
        assert!(!handler0.handle_authentication(authentication));
        let missing_nodes = handler0.missing_nodes();
        let expected_missing: Vec<_> = (1..NUM_NODES).map(NodeIndex).collect();
        assert_eq!(missing_nodes, expected_missing);
    }

    #[tokio::test]
    async fn ignores_wrong_session_authentication() {
        let keyboxes_components = keyboxes_components().await;
        let mut handler0 = Handler::new(
            keyboxes_components[0].0.clone(),
            keyboxes_components[0].1.clone(),
            SessionId(43),
            correct_addresses_0(),
        )
        .await
        .unwrap();
        let handler1 = Handler::new(
            keyboxes_components[1].0.clone(),
            keyboxes_components[1].1.clone(),
            SessionId(44),
            correct_addresses_1(),
        )
        .await
        .unwrap();
        assert!(!handler0.handle_authentication(handler1.authentication().unwrap()));
        let missing_nodes = handler0.missing_nodes();
        let expected_missing: Vec<_> = (1..NUM_NODES).map(NodeIndex).collect();
        assert_eq!(missing_nodes, expected_missing);
    }

    #[tokio::test]
    async fn ignores_own_authentication() {
        let awaited_keyboxes_components = keyboxes_components().await;
        let mut handler0 = Handler::new(
            awaited_keyboxes_components[0].0.clone(),
            awaited_keyboxes_components[0].1.clone(),
            SessionId(43),
            correct_addresses_0(),
        )
        .await
        .unwrap();
        assert!(!handler0.handle_authentication(handler0.authentication().unwrap()));
        let missing_nodes = handler0.missing_nodes();
        let expected_missing: Vec<_> = (1..NUM_NODES).map(NodeIndex).collect();
        assert_eq!(missing_nodes, expected_missing);
    }

    #[tokio::test]
    async fn invalidates_obsolete_authentication() {
        let awaited_keyboxes_components = keyboxes_components().await;
        let mut handler0 = Handler::new(
            awaited_keyboxes_components[0].0.clone(),
            awaited_keyboxes_components[0].1.clone(),
            SessionId(43),
            correct_addresses_0(),
        )
        .await
        .unwrap();
        let handler1 = Handler::new(
            awaited_keyboxes_components[1].0.clone(),
            awaited_keyboxes_components[1].1.clone(),
            SessionId(43),
            correct_addresses_1(),
        )
        .await
        .unwrap();
        assert!(handler0.handle_authentication(handler1.authentication().unwrap()));
        let new_keyboxes_components = keyboxes_components().await;
        print!(
            "{:?}",
            handler0
                .update(
                    new_keyboxes_components[0].0.clone(),
                    new_keyboxes_components[0].1.clone(),
                    correct_addresses_0()
                )
                .await
                .unwrap()
        );
        let missing_nodes = handler0.missing_nodes();
        let expected_missing: Vec<_> = (1..NUM_NODES).map(NodeIndex).collect();
        assert_eq!(missing_nodes, expected_missing);
        assert!(handler0.peer_id(&NodeIndex(1)).is_none());
    }

    #[tokio::test]
    async fn uses_cached_authentication() {
        let awaited_keyboxes_components = keyboxes_components().await;
        let mut handler0 = Handler::new(
            awaited_keyboxes_components[0].0.clone(),
            awaited_keyboxes_components[0].1.clone(),
            SessionId(43),
            correct_addresses_0(),
        )
        .await
        .unwrap();
        let mut handler1 = Handler::new(
            awaited_keyboxes_components[1].0.clone(),
            awaited_keyboxes_components[1].1.clone(),
            SessionId(43),
            correct_addresses_1(),
        )
        .await
        .unwrap();
        assert!(handler0.handle_authentication(handler1.authentication().unwrap()));
        let new_keyboxes_components = keyboxes_components().await;
        assert!(handler1
            .update(
                new_keyboxes_components[1].0.clone(),
                new_keyboxes_components[1].1.clone(),
                correct_addresses_1()
            )
            .await
            .unwrap()
            .is_empty());
        assert!(!handler0.handle_authentication(handler1.authentication().unwrap()));
        handler0
            .update(
                new_keyboxes_components[0].0.clone(),
                new_keyboxes_components[0].1.clone(),
                correct_addresses_0(),
            )
            .await
            .unwrap();
        let missing_nodes = handler0.missing_nodes();
        let expected_missing: Vec<_> = (2..NUM_NODES).map(NodeIndex).collect();
        assert_eq!(missing_nodes, expected_missing);
        assert_eq!(
            handler0.peer_id(&NodeIndex(1)),
            get_common_peer_id(&correct_addresses_1())
        );
    }
}
