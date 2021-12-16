use crate::new_network::{ComponentNetwork, Data, ReceiverComponent, SendError, SenderComponent};
use aleph_bft::Recipient;
use codec::{Decode, Encode};
use futures::channel::mpsc;
use log::warn;
use std::{marker::PhantomData, sync::Arc};
use tokio::{
    sync::Mutex,
    time::{interval, Duration},
};

const MAXIMUM_RETRY_MS: u64 = 500;

/// Used for routing data through split networks.
#[derive(Clone, Encode, Decode)]
pub enum Split<D1: Data, D2: Data> {
    Left(D1),
    Right(D2),
}

#[derive(Clone)]
struct LeftSender<D1: Data, D2: Data, S: SenderComponent<Split<D1, D2>>> {
    sender: S,
    phantom: PhantomData<(D1, D2)>,
}

impl<D1: Data, D2: Data, S: SenderComponent<Split<D1, D2>>> SenderComponent<D1>
    for LeftSender<D1, D2, S>
{
    fn send(&self, data: D1, recipient: Recipient) -> Result<(), SendError> {
        self.sender.send(Split::Left(data), recipient)
    }
}

#[derive(Clone)]
struct RightSender<D1: Data, D2: Data, S: SenderComponent<Split<D1, D2>>> {
    sender: S,
    phantom: PhantomData<(D1, D2)>,
}

impl<D1: Data, D2: Data, S: SenderComponent<Split<D1, D2>>> SenderComponent<D2>
    for RightSender<D1, D2, S>
{
    fn send(&self, data: D2, recipient: Recipient) -> Result<(), SendError> {
        self.sender.send(Split::Right(data), recipient)
    }
}

struct LeftReceiver<D1: Data, D2: Data, R: ReceiverComponent<Split<D1, D2>>> {
    receiver: Arc<Mutex<R>>,
    translated_receiver: mpsc::UnboundedReceiver<D1>,
    left_sender: mpsc::UnboundedSender<D1>,
    right_sender: mpsc::UnboundedSender<D2>,
}

struct RightReceiver<D1: Data, D2: Data, R: ReceiverComponent<Split<D1, D2>>> {
    receiver: Arc<Mutex<R>>,
    translated_receiver: mpsc::UnboundedReceiver<D2>,
    left_sender: mpsc::UnboundedSender<D1>,
    right_sender: mpsc::UnboundedSender<D2>,
}

async fn forward_or_wait<D1: Data, D2: Data, R: ReceiverComponent<Split<D1, D2>>>(
    receiver: &Arc<Mutex<R>>,
    left_sender: &mpsc::UnboundedSender<D1>,
    right_sender: &mpsc::UnboundedSender<D2>,
) {
    match receiver.try_lock().ok() {
        Some(mut receiver) => match receiver.next().await {
            Some(Split::Left(data)) => {
                if left_sender.unbounded_send(data).is_err() {
                    warn!(target: "aleph-network", "Failed send despite controlling receiver, this shouldn't've happened.");
                }
            }
            Some(Split::Right(data)) => {
                if right_sender.unbounded_send(data).is_err() {
                    warn!(target: "aleph-network", "Failed send despite controlling receiver, this shouldn't've happened.");
                }
            }
            None => {
                left_sender.close_channel();
                right_sender.close_channel();
            }
        },
        None => {
            interval(Duration::from_millis(MAXIMUM_RETRY_MS))
                .tick()
                .await;
        }
    }
}

#[async_trait::async_trait]
impl<D1: Data, D2: Data, R: ReceiverComponent<Split<D1, D2>>> ReceiverComponent<D1>
    for LeftReceiver<D1, D2, R>
{
    async fn next(&mut self) -> Option<D1> {
        loop {
            tokio::select! {
                data = self.translated_receiver.next() => return data,
                _ = forward_or_wait(&self.receiver, &self.left_sender, &self.right_sender) => (),
            }
        }
    }
}

#[async_trait::async_trait]
impl<D1: Data, D2: Data, R: ReceiverComponent<Split<D1, D2>>> ReceiverComponent<D2>
    for RightReceiver<D1, D2, R>
{
    async fn next(&mut self) -> Option<D2> {
        loop {
            tokio::select! {
                data = self.translated_receiver.next() => return data,
                _ = forward_or_wait(&self.receiver, &self.left_sender, &self.right_sender) => (),
            }
        }
    }
}

struct LeftNetwork<
    D1: Data,
    D2: Data,
    S: SenderComponent<Split<D1, D2>>,
    R: ReceiverComponent<Split<D1, D2>>,
> {
    sender: LeftSender<D1, D2, S>,
    receiver: Arc<Mutex<LeftReceiver<D1, D2, R>>>,
}

impl<
        D1: Data,
        D2: Data,
        S: SenderComponent<Split<D1, D2>>,
        R: ReceiverComponent<Split<D1, D2>>,
    > ComponentNetwork<D1> for LeftNetwork<D1, D2, S, R>
{
    type S = LeftSender<D1, D2, S>;
    type R = LeftReceiver<D1, D2, R>;
    fn sender(&self) -> &Self::S {
        &self.sender
    }
    fn receiver(&self) -> Arc<Mutex<Self::R>> {
        self.receiver.clone()
    }
}

struct RightNetwork<
    D1: Data,
    D2: Data,
    S: SenderComponent<Split<D1, D2>>,
    R: ReceiverComponent<Split<D1, D2>>,
> {
    sender: RightSender<D1, D2, S>,
    receiver: Arc<Mutex<RightReceiver<D1, D2, R>>>,
}

impl<
        D1: Data,
        D2: Data,
        S: SenderComponent<Split<D1, D2>>,
        R: ReceiverComponent<Split<D1, D2>>,
    > ComponentNetwork<D2> for RightNetwork<D1, D2, S, R>
{
    type S = RightSender<D1, D2, S>;
    type R = RightReceiver<D1, D2, R>;
    fn sender(&self) -> &Self::S {
        &self.sender
    }
    fn receiver(&self) -> Arc<Mutex<Self::R>> {
        self.receiver.clone()
    }
}

fn split_sender<D1: Data, D2: Data, S: SenderComponent<Split<D1, D2>>>(
    sender: &S,
) -> (LeftSender<D1, D2, S>, RightSender<D1, D2, S>) {
    (
        LeftSender {
            sender: sender.clone(),
            phantom: PhantomData,
        },
        RightSender {
            sender: sender.clone(),
            phantom: PhantomData,
        },
    )
}

fn split_receiver<D1: Data, D2: Data, R: ReceiverComponent<Split<D1, D2>>>(
    receiver: Arc<Mutex<R>>,
) -> (LeftReceiver<D1, D2, R>, RightReceiver<D1, D2, R>) {
    let (left_sender, left_receiver) = mpsc::unbounded();
    let (right_sender, right_receiver) = mpsc::unbounded();
    (
        LeftReceiver {
            receiver: receiver.clone(),
            translated_receiver: left_receiver,
            left_sender: left_sender.clone(),
            right_sender: right_sender.clone(),
        },
        RightReceiver {
            receiver,
            translated_receiver: right_receiver,
            left_sender,
            right_sender,
        },
    )
}

/// Split a single component network into two separate ones. This way multiple components can send
/// data to the same underlying session not knowing what types of data the other ones use.
///
/// The main example for now is creating an `aleph_bft::Network` and a separate one for accumulating
/// signatures for justifications.
pub fn split<D1: Data, D2: Data, CN: ComponentNetwork<Split<D1, D2>>>(
    network: CN,
) -> (impl ComponentNetwork<D1>, impl ComponentNetwork<D2>) {
    let (left_sender, right_sender) = split_sender(network.sender());
    let (left_receiver, right_receiver) = split_receiver(network.receiver());
    (
        LeftNetwork {
            sender: left_sender,
            receiver: Arc::new(Mutex::new(left_receiver)),
        },
        RightNetwork {
            sender: right_sender,
            receiver: Arc::new(Mutex::new(right_receiver)),
        },
    )
}
