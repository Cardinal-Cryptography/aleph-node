use crate::new_network::{Data, DataNetwork, SendError};
use aleph_bft::Recipient;
use futures::channel::mpsc;
use std::sync::Arc;
use tokio::{stream::StreamExt, sync::Mutex};

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
        self.receiver().clone().lock_owned().await.next().await
    }
}

#[async_trait::async_trait]
impl<D: Data> Receiver<D> for mpsc::UnboundedReceiver<D> {
    async fn next(&mut self) -> Option<D> {
        StreamExt::next(self).await
    }
}

#[cfg(test)]
mod tests {
    use super::{Network, Receiver, Sender};
    use crate::new_network::{SendError};
    use aleph_bft::Recipient;
    use futures::channel::mpsc;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use std::ops::DerefMut;

    impl Sender<u64> for mpsc::UnboundedSender<u64> {
        fn send(&self, data: u64, _recipient: Recipient) -> Result<(), SendError> {
            self.unbounded_send(data).map_err(|_| SendError::SendFailed)
        }
    }

    struct TestNetwork {
        sender: mpsc::UnboundedSender<u64>,
        receiver: Arc<Mutex<mpsc::UnboundedReceiver<u64>>>,
    }

    impl Network<u64> for TestNetwork {
        type S = mpsc::UnboundedSender<u64>;

        type R = mpsc::UnboundedReceiver<u64>;
        
        fn sender(&self) -> &Self::S {
            &self.sender
        }
        
        fn receiver(&self) -> Arc<Mutex<Self::R>> {
            self.receiver.clone()
        }
    }

    #[tokio::test]
    async fn test_receiver_implementation() {
        let (sender, receiver) = mpsc::unbounded();
        let network = TestNetwork {
            sender,
            receiver: Arc::new(Mutex::new(receiver)),
        };

        let sender = network.sender();
        let mut receiver = network.receiver().lock_owned().await; 

        let val = 1234;
        Sender::<u64>::send(sender, val, Recipient::Everyone).unwrap();
        let received = Receiver::<u64>::next(receiver.deref_mut()).await;
        assert_eq!(Some(val), received);
    }
}