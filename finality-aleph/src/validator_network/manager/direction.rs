use std::collections::{HashMap, HashSet};

use aleph_primitives::AuthorityId;

use crate::validator_network::Data;

pub struct DirectedPeers<A: Data> {
    own_id: AuthorityId,
    outgoing: HashMap<AuthorityId, Vec<A>>,
    incoming: HashSet<AuthorityId>,
}

fn bit_xor_sum_parity((a, b): (u8, u8)) -> u8 {
    let mut result = 0;
    for i in 0..8 {
        result += ((a >> i) ^ (b >> i)) % 2;
    }
    result % 2
}

// Whether we shold call the remote or the other way around. We xor the peer ids and based on the
// parity of the sum of bits of the result decide whether the caller should be the smaller or
// greated lexicographically. They are never equal, because cryptography.
fn should_call(own_id: &[u8], remote_id: &[u8]) -> bool {
    let xor_sum_parity: u8 = own_id
        .iter()
        .cloned()
        .zip(remote_id.iter().cloned())
        .map(bit_xor_sum_parity)
        .fold(0u8, |a, b| (a + b) % 2);
    match xor_sum_parity == 0 {
        true => own_id < remote_id,
        false => own_id > remote_id,
    }
}

impl<A: Data> DirectedPeers<A> {
    /// Create a new set of peers directed using our own peer id.
    pub fn new(own_id: AuthorityId) -> Self {
        DirectedPeers {
            own_id,
            outgoing: HashMap::new(),
            incoming: HashSet::new(),
        }
    }

    /// Add a peer to the list of peers we want to stay connected to, or
    /// update the list of addresses if the peer was already added.
    /// Returns whether we should start attampts at connecting with the peer, which is the case
    /// exactly when the peer is one with which we should attempt connections AND it was added for
    /// the first time.
    pub fn add_peer(&mut self, peer_id: AuthorityId, addresses: Vec<A>) -> bool {
        match should_call(self.own_id.as_ref(), peer_id.as_ref()) {
            true => self.outgoing.insert(peer_id, addresses).is_none(),
            false => {
                // We discard the addresses here, as we will never want to call this peer anyway,
                // so we don't need them.
                self.incoming.insert(peer_id);
                false
            }
        }
    }

    /// Return the addresses of the given peer, or None if we shouldn't attempt connecting with the peer.
    pub fn peer_addresses(&self, peer_id: &AuthorityId) -> Option<Vec<A>> {
        self.outgoing.get(peer_id).cloned()
    }

    /// Whether we should be maintaining a connection with this peer.
    pub fn interested(&self, peer_id: &AuthorityId) -> bool {
        self.incoming.contains(peer_id) || self.outgoing.contains_key(peer_id)
    }

    /// Iterator over the peers we want connections from.
    pub fn incoming_peers(&self) -> impl Iterator<Item = &AuthorityId> {
        self.incoming.iter()
    }

    /// Iterator over the peers we want to connect to.
    pub fn outgoing_peers(&self) -> impl Iterator<Item = &AuthorityId> {
        self.outgoing.keys()
    }

    /// Remove a peer from the list of peers that we want to stay connected with, whether the
    /// connection was supposed to be incoming or outgoing.
    pub fn remove_peer(&mut self, peer_id: &AuthorityId) {
        self.incoming.remove(peer_id);
        self.outgoing.remove(peer_id);
    }
}

#[cfg(test)]
mod tests {
    use aleph_primitives::AuthorityId;

    use super::DirectedPeers;
    use crate::validator_network::mock::key;

    type Address = String;

    async fn container_with_id() -> (DirectedPeers<Address>, AuthorityId) {
        let (own_id, _) = key().await;
        let own_container = DirectedPeers::new(own_id.clone());
        (own_container, own_id)
    }

    #[tokio::test]
    async fn exactly_one_direction_attempts_connections() {
        let (mut own_container, own_id) = container_with_id().await;
        let (mut remote_container, remote_id) = container_with_id().await;
        let addresses = vec![
            String::from(""),
            String::from("a/b/c"),
            String::from("43.43.43.43:43000"),
        ];
        assert!(
            own_container.add_peer(remote_id, addresses.clone())
                != remote_container.add_peer(own_id, addresses.clone())
        );
    }

    async fn container_with_added_connecting_peer() -> (DirectedPeers<Address>, AuthorityId) {
        let (mut own_container, own_id) = container_with_id().await;
        let (mut remote_container, remote_id) = container_with_id().await;
        let addresses = vec![
            String::from(""),
            String::from("a/b/c"),
            String::from("43.43.43.43:43000"),
        ];
        match own_container.add_peer(remote_id.clone(), addresses.clone()) {
            true => (own_container, remote_id),
            false => {
                remote_container.add_peer(own_id.clone(), addresses);
                (remote_container, own_id)
            }
        }
    }

    async fn container_with_added_nonconnecting_peer() -> (DirectedPeers<Address>, AuthorityId) {
        let (mut own_container, own_id) = container_with_id().await;
        let (mut remote_container, remote_id) = container_with_id().await;
        let addresses = vec![
            String::from(""),
            String::from("a/b/c"),
            String::from("43.43.43.43:43000"),
        ];
        match own_container.add_peer(remote_id.clone(), addresses.clone()) {
            false => (own_container, remote_id),
            true => {
                remote_container.add_peer(own_id.clone(), addresses);
                (remote_container, own_id)
            }
        }
    }

    #[tokio::test]
    async fn no_connecting_on_readd() {
        let (mut own_container, remote_id) = container_with_added_connecting_peer().await;
        let addresses = vec![
            String::from(""),
            String::from("a/b/c"),
            String::from("43.43.43.43:43000"),
        ];
        assert!(!own_container.add_peer(remote_id, addresses));
    }

    #[tokio::test]
    async fn peer_addresses_when_connecting() {
        let (own_container, remote_id) = container_with_added_connecting_peer().await;
        assert!(own_container.peer_addresses(&remote_id).is_some());
    }

    #[tokio::test]
    async fn no_peer_addresses_when_nonconnecting() {
        let (own_container, remote_id) = container_with_added_nonconnecting_peer().await;
        assert!(own_container.peer_addresses(&remote_id).is_none());
    }

    #[tokio::test]
    async fn interested_in_connecting() {
        let (own_container, remote_id) = container_with_added_connecting_peer().await;
        assert!(own_container.interested(&remote_id));
    }

    #[tokio::test]
    async fn interested_in_nonconnecting() {
        let (own_container, remote_id) = container_with_added_nonconnecting_peer().await;
        assert!(own_container.interested(&remote_id));
    }

    #[tokio::test]
    async fn uninterested_in_unknown() {
        let (own_container, _) = container_with_id().await;
        let (_, remote_id) = container_with_id().await;
        assert!(!own_container.interested(&remote_id));
    }

    #[tokio::test]
    async fn connecting_are_outgoing() {
        let (own_container, remote_id) = container_with_added_connecting_peer().await;
        assert_eq!(
            own_container.outgoing_peers().collect::<Vec<_>>(),
            vec![&remote_id]
        );
        assert_eq!(own_container.incoming_peers().next(), None);
    }

    #[tokio::test]
    async fn nonconnecting_are_incoming() {
        let (own_container, remote_id) = container_with_added_nonconnecting_peer().await;
        assert_eq!(
            own_container.incoming_peers().collect::<Vec<_>>(),
            vec![&remote_id]
        );
        assert_eq!(own_container.outgoing_peers().next(), None);
    }

    #[tokio::test]
    async fn uninterested_in_removed() {
        let (mut own_container, remote_id) = container_with_added_connecting_peer().await;
        assert!(own_container.interested(&remote_id));
        own_container.remove_peer(&remote_id);
        assert!(!own_container.interested(&remote_id));
    }
}
