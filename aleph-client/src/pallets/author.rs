use codec::Decode;

use crate::{aleph_runtime::SessionKeys, ConnectionExt};

#[async_trait::async_trait]
pub trait AuthorRpc {
    async fn author_rotate_keys(&self) -> SessionKeys;
}

#[async_trait::async_trait]
impl<C: ConnectionExt> AuthorRpc for C {
    async fn author_rotate_keys(&self) -> SessionKeys {
        let bytes = self.as_connection().rpc().rotate_keys().await.unwrap();

        SessionKeys::decode(&mut bytes.0.as_slice()).unwrap()
    }
}
