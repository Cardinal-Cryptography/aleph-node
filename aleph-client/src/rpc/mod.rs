use codec::Decode;

use crate::{aleph_runtime::SessionKeys, SignedConnection};

#[async_trait::async_trait]
pub trait Rpc {
    async fn author_rotate_keys(&self) -> SessionKeys;
}

#[async_trait::async_trait]
impl Rpc for SignedConnection {
    async fn author_rotate_keys(&self) -> SessionKeys {
        let bytes = self.connection.client.rpc().rotate_keys().await.unwrap();

        SessionKeys::decode(&mut bytes.0.as_slice()).unwrap()
    }
}
