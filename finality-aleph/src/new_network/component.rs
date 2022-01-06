use crate::new_network::{Data, DataNetwork, SendError};
use aleph_bft::Recipient;
use futures::channel::mpsc;
use std::sync::Arc;
use tokio::sync::Mutex;
use log::trace;
use futures::StreamExt;

/// For sending arbitrary messages.
pub trait Sender<D: Data>: Sync + Send + Clone {
    fn send(&self, data: D, recipient: Recipient) -> Result<(), SendError>;
}

/// For receiving arbitrary messages.
#[async_trait::async_trait]
pub trait Receiver<D: Data>: Sync + Send {
    async fn next(&mut self) -> Option<D>;
}

/// A bare version of network components.
pub trait Network<D: Data>: Sync + Send {
    type S: Sender<D>;
    type R: Receiver<D>;
    fn sender(&self) -> &Self::S;
    fn receiver(&self) -> Arc<Mutex<Self::R>>;
}

#[async_trait::async_trait]
impl<D: Data, CN: Network<D>> DataNetwork<D> for CN {
    fn send(&self, data: D, recipient: Recipient) -> Result<(), SendError> {
        self.sender().send(data, recipient)
    }
    async fn next(&mut self) -> Option<D> {
        trace!(target: "aleph-network", "Component DataNetwork Next");
        let r = self.receiver();
        trace!(target: "aleph-network", "Component DataNetwork Next Cloned");
        let mut q = r.lock_owned().await;
        trace!(target: "aleph-network", "Component DataNetwork Next Locked");
        let s = q.next().await;
        trace!(target: "aleph-network", "Component DataNetwork Next Nexted");
        s
    }
}

#[async_trait::async_trait]
impl<D: Data> Receiver<D> for mpsc::UnboundedReceiver<D> {
    async fn next(&mut self) -> Option<D> {
        StreamExt::next(self).await
    }
}
