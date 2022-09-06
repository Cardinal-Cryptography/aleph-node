use std::{fmt::Display, marker::PhantomData};

use aleph_bft::Recipient;
use futures::{channel::mpsc, StreamExt};
use log::warn;

use crate::network::{Data, DataNetwork, SendError};

/// For sending arbitrary messages.
pub trait Sender<D: Data>: Sync + Send + Clone {
    fn send(&self, data: D, recipient: Recipient) -> Result<(), SendError>;
}

#[derive(Clone)]
pub struct MapSender<D, S> {
    sender: S,
    _phantom: PhantomData<D>,
}

pub trait SenderMap<FromData: Data>: Sender<FromData> {
    fn map(self) -> MapSender<FromData, Self> {
        MapSender {
            sender: self,
            _phantom: PhantomData,
        }
    }
}

impl<D: Data, S: Sender<D>> SenderMap<D> for S {}

impl<D: Data, S: Sender<D>, IntoD: Data + Into<D>> Sender<IntoD> for MapSender<D, S> {
    fn send(&self, data: IntoD, recipient: Recipient) -> Result<(), SendError> {
        self.sender.send(data.into(), recipient)
    }
}

/// For receiving arbitrary messages.
#[async_trait::async_trait]
pub trait Receiver<D: Data>: Sync + Send {
    async fn next(&mut self) -> Option<D>;
}

pub struct MapReceiver<D, R> {
    receiver: R,
    _phantom: PhantomData<D>,
}

pub trait ReceiverMap<FromData: Data>: Receiver<FromData> + Sized {
    fn map(self) -> MapReceiver<FromData, Self> {
        MapReceiver {
            receiver: self,
            _phantom: PhantomData,
        }
    }
}

impl<D: Data, R: Receiver<D>> ReceiverMap<D> for R {}

#[async_trait::async_trait]
impl<D: Data, R: Receiver<D>, FromD: Data + TryFrom<D>> Receiver<FromD> for MapReceiver<D, R>
where
    FromD::Error: Display,
{
    async fn next(&mut self) -> Option<FromD> {
        loop {
            let data = self.receiver.next().await;
            let data = match data {
                Some(data) => data,
                None => return None,
            };
            match TryFrom::try_from(data) {
                Ok(message) => return Some(message),
                Err(e) => {
                    warn!(target: "aleph-network", "Error decoding message in MapReceiver: {}", e)
                }
            }
        }
    }
}

/// A bare version of network components.
pub trait Network<D: Data>: Sync + Send {
    type S: Sender<D>;
    type R: Receiver<D>;

    fn into(self) -> (Self::S, Self::R);
}

pub trait NetworkExt<D: Data>: Network<D> + AsRef<Self::S> + AsMut<Self::R> {}

impl<D: Data, N: Network<D> + AsRef<N::S> + AsMut<N::R>> NetworkExt<D> for N {}

#[async_trait::async_trait]
impl<D: Data, N: NetworkExt<D>> DataNetwork<D> for N {
    fn send(&self, data: D, recipient: Recipient) -> Result<(), SendError> {
        self.as_ref().send(data, recipient)
    }

    async fn next(&mut self) -> Option<D> {
        self.as_mut().next().await
    }
}

pub trait NetworkMap<D: Data, IntoD: Data>: Network<D> {
    type MappedNetwork: Network<IntoD>;

    fn map(self) -> Self::MappedNetwork;
}

impl<D: Data, IntoD: Data + Into<D> + TryFrom<D>, N: Network<D>> NetworkMap<D, IntoD> for N
where
    IntoD::Error: Display,
{
    type MappedNetwork = SimpleNetwork<IntoD, MapReceiver<D, N::R>, MapSender<D, N::S>>;

    fn map(self) -> Self::MappedNetwork {
        let (sender, receiver) = self.into();
        SimpleNetwork::new(receiver.map(), sender.map())
    }
}

#[async_trait::async_trait]
impl<D: Data> Sender<D> for mpsc::UnboundedSender<(D, Recipient)> {
    fn send(&self, data: D, recipient: Recipient) -> Result<(), SendError> {
        self.unbounded_send((data, recipient))
            .map_err(|_| SendError::SendFailed)
    }
}

#[async_trait::async_trait]
impl<D: Data> Receiver<D> for mpsc::UnboundedReceiver<D> {
    async fn next(&mut self) -> Option<D> {
        StreamExt::next(self).await
    }
}

pub struct SimpleNetwork<D: Data, R: Receiver<D>, S: Sender<D>> {
    receiver: R,
    sender: S,
    _phantom: PhantomData<D>,
}

impl<D: Data, R: Receiver<D>, S: Sender<D>> SimpleNetwork<D, R, S> {
    pub fn new(receiver: R, sender: S) -> Self {
        SimpleNetwork {
            receiver,
            sender,
            _phantom: PhantomData,
        }
    }
}

impl<D: Data, R: Receiver<D>, S: Sender<D>> AsRef<S> for SimpleNetwork<D, R, S> {
    fn as_ref(&self) -> &S {
        &self.sender
    }
}

impl<D: Data, R: Receiver<D>, S: Sender<D>> AsMut<R> for SimpleNetwork<D, R, S> {
    fn as_mut(&mut self) -> &mut R {
        &mut self.receiver
    }
}

impl<D: Data, R: Receiver<D>, S: Sender<D>> Network<D> for SimpleNetwork<D, R, S> {
    type S = S;

    type R = R;

    fn into(self) -> (Self::S, Self::R) {
        (self.sender, self.receiver)
    }
}

#[cfg(test)]
mod tests {
    use aleph_bft::Recipient;
    use futures::channel::mpsc;

    use super::{DataNetwork, NetworkMap, Receiver, Sender, SimpleNetwork};
    use crate::network::component::{Network, ReceiverMap, SenderMap};

    #[tokio::test]
    async fn test_receiver_implementation() {
        let (sender, mut receiver) = mpsc::unbounded();

        let val = 1234;
        sender.unbounded_send(val).unwrap();
        let received = Receiver::<u64>::next(&mut receiver).await;
        assert_eq!(Some(val), received);
    }

    enum FromType {
        A,
        B,
    }

    struct IntoType {}

    impl TryFrom<FromType> for IntoType {
        type Error = ();

        fn try_from(value: FromType) -> Result<Self, Self::Error> {
            match value {
                FromType::A => Ok(IntoType {}),
                FromType::B => Err(()),
            }
        }
    }

    impl Into<FromType> for IntoType {
        fn into(self) -> FromType {
            FromType::A
        }
    }

    #[tokio::test]
    async fn test_receiver_map_allows_to_receive_mapped_data() {
        let (sender, receiver) = mpsc::unbounded();

        let from_data = FromType::A;
        let into_data = IntoType {};
        let mut mapped_receiver = receiver.map();

        sender.unbounded_send(from_data).unwrap();

        let received = Receiver::next(&mut mapped_receiver).await;
        assert_eq!(Some(into_data), received);
    }

    #[tokio::test]
    async fn test_map_allows_to_send_and_receive_mapped_data() {
        let (sender, receiver) = mpsc::unbounded();

        let from_data = FromType::A;
        let into_data = IntoType {};
        let mapped_sender = sender.map();

        mapped_sender.send(into_data, Recipient::Everyone);

        let received = receiver.recv().await;
        assert_eq!(Some(from_data), received);
    }

    #[tokio::test]
    async fn test_mapped_receiver_only_returns_convertable_values() {
        let (sender, receiver) = mpsc::unbounded();

        let from_data = FromType::A;
        let into_data = IntoType {};
        let mut mapped_receiver = receiver.map();

        sender.unbounded_send(FromType::B).unwrap();
        sender.unbounded_send(FromType::B).unwrap();
        sender.unbounded_send(from_data).unwrap();
        sender.close_channel();

        let received = receiver.recv().await;
        assert_eq!(Some(into_type), received);
        let received = receiver.recv().await;
        assert_eq!(None, received);
    }

    #[tokio::test]
    async fn test_mapped_networks_are_able_to_send_and_receive_data() {
        let (sender_for_network, receiver) = mpsc::unbounded();
        let (sender, receiver_for_network) = mpsc::unbounded();

        let network = SimpleNetwork::new(receiver_for_receiver, sender_for_network);
        let other_network = SimpleNetwork::new(receiver, sender);
        let mapped_network = other_network.map();

        let from_data = FromType::A;
        let into_data = IntoType {};

        network.send(from_data, Recipient::Everyone).unwrap();
        let received = mapped_network.next().await;
        assert_eq!(into_data, received);

        mapped_network.send(into_data, Recipient::Everyone).unwrap();
        let received = network.next().await;
        assert_eq!(Some(from_data), received);
    }
}
