use std::sync::Arc;

use aleph_primitives::{AuthorityId, KEY_TYPE};
use sp_keystore::{testing::KeyStore, CryptoStore};

use crate::{
    network::{mock::Channel, Data, Multiaddress, NetworkIdentity, PeerId},
    validator_network::Network,
};

type MockPeerId = AuthorityId;

impl PeerId for MockPeerId {}

type MockMultiaddress = (AuthorityId, String);

impl Multiaddress for MockMultiaddress {
    type PeerId = AuthorityId;

    fn get_peer_id(&self) -> Option<Self::PeerId> {
        Some(self.0.clone())
    }

    fn add_matching_peer_id(self, peer_id: Self::PeerId) -> Option<Self> {
        match self.0 == peer_id {
            true => Some(self),
            false => None,
        }
    }
}

pub struct MockNetwork<D: Data> {
    pub add_connection: Channel<(MockPeerId, Vec<MockMultiaddress>)>,
    pub remove_connection: Channel<MockPeerId>,
    pub send: Channel<(D, MockPeerId)>,
    pub next: Channel<D>,
    id: MockPeerId,
    addresses: Vec<MockMultiaddress>,
}

#[async_trait::async_trait]
impl<D: Data> Network<MockMultiaddress, D> for MockNetwork<D> {
    fn add_connection(&mut self, peer: MockPeerId, addresses: Vec<MockMultiaddress>) {
        self.add_connection.send((peer, addresses));
    }

    fn remove_connection(&mut self, peer: MockPeerId) {
        self.remove_connection.send(peer);
    }

    fn send(&self, data: D, recipient: MockPeerId) {
        self.send.send((data, recipient));
    }

    async fn next(&mut self) -> Option<D> {
        self.next.next().await
    }
}

impl<D: Data> NetworkIdentity for MockNetwork<D> {
    type PeerId = MockPeerId;
    type Multiaddress = MockMultiaddress;

    fn identity(&self) -> (Vec<Self::Multiaddress>, Self::PeerId) {
        (self.addresses.clone(), self.id.clone())
    }
}

impl<D: Data> MockNetwork<D> {
    pub async fn _new(address: &str) -> Self {
        let key_store = Arc::new(KeyStore::new());
        let id: AuthorityId = key_store
            .ed25519_generate_new(KEY_TYPE, None)
            .await
            .unwrap()
            .into();
        let addresses = vec![(id.clone(), String::from(address))];
        MockNetwork {
            add_connection: Channel::new(),
            remove_connection: Channel::new(),
            send: Channel::new(),
            next: Channel::new(),
            addresses,
            id,
        }
    }

    // Consumes the network asserting there are no unreceived messages in the channels.
    pub async fn _close_channels(self) {
        assert!(self.add_connection.close().await.is_none());
        assert!(self.remove_connection.close().await.is_none());
        assert!(self.send.close().await.is_none());
        assert!(self.next.close().await.is_none());
    }
}
