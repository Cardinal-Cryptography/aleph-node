use codec::{Encode, Decode};
use aleph_primitives::AuthorityId;

/// Network represents an interface for opening and closing connections with other Validators,
/// and sending direct messages between them.
///
/// Note on Network reliability and security: it is neither assumed that the sent messages must be
/// always delivered, nor the established connections must be secure in any way. If the Network
/// implementation fails to deliver a message, it may assume that the send method will be called
/// again.
#[async_trait::async_trait]
pub trait Network<D>: Send {
    /// Add the peer to the set of connected peers.
    fn add_connection(&mut self, peer: AuthorityId, addresses: Vec<String>);

    /// Remove the peer from the set of connected peers and close the connection.
    fn del_connection(&mut self, peer: AuthorityId);

    /// Send a message to a single peer.
    /// Note on the implementation: this function should be implemented in a non-blocking manner.
    fn send(&self, data: D, recipient: AuthorityId);

    /// Receive a message from the network.
    async fn next(&mut self) -> D;
}

pub struct MockNetwork;
#[derive(Encode, Decode)]
pub struct MockData;

#[async_trait::async_trait]
impl Network<MockData> for MockNetwork {
    fn add_connection(&mut self, peer: AuthorityId, addresses: Vec<String>) {}

    fn del_connection(&mut self, peer: AuthorityId) {}

    fn send(&self, data: MockData, recipient: AuthorityId) {}

    async fn next(&mut self) -> MockData {
        MockData
    }
}
