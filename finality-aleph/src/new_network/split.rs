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

/// Used for routing data through split networks.
#[derive(Clone, Encode, Decode)]
pub enum Split<D1: Data, D2: Data> {
    Hidari(D1),
    Migi(D2),
}

#[derive(Clone)]
struct HidariSender<D1: Data, D2: Data, S: SenderComponent<Split<D1, D2>>> {
    sender: S,
    d1: PhantomData<D1>,
    d2: PhantomData<D2>,
}

impl<D1: Data, D2: Data, S: SenderComponent<Split<D1, D2>>> SenderComponent<D1>
    for HidariSender<D1, D2, S>
{
    fn send(&self, data: D1, recipient: Recipient) -> Result<(), SendError> {
        self.sender.send(Split::Hidari(data), recipient)
    }
}

#[derive(Clone)]
struct MigiSender<D1: Data, D2: Data, S: SenderComponent<Split<D1, D2>>> {
    sender: S,
    d1: PhantomData<D1>,
    d2: PhantomData<D2>,
}

impl<D1: Data, D2: Data, S: SenderComponent<Split<D1, D2>>> SenderComponent<D2>
    for MigiSender<D1, D2, S>
{
    fn send(&self, data: D2, recipient: Recipient) -> Result<(), SendError> {
        self.sender.send(Split::Migi(data), recipient)
    }
}

struct HidariReceiver<D1: Data, D2: Data, R: ReceiverComponent<Split<D1, D2>>> {
    receiver: Arc<Mutex<R>>,
    translated_receiver: mpsc::UnboundedReceiver<D1>,
    hidari_sender: mpsc::UnboundedSender<D1>,
    migi_sender: mpsc::UnboundedSender<D2>,
}

struct MigiReceiver<D1: Data, D2: Data, R: ReceiverComponent<Split<D1, D2>>> {
    receiver: Arc<Mutex<R>>,
    translated_receiver: mpsc::UnboundedReceiver<D2>,
    hidari_sender: mpsc::UnboundedSender<D1>,
    migi_sender: mpsc::UnboundedSender<D2>,
}

async fn forward_or_wait<D1: Data, D2: Data, R: ReceiverComponent<Split<D1, D2>>>(
    receiver: &Arc<Mutex<R>>,
    hidari_sender: &mpsc::UnboundedSender<D1>,
    migi_sender: &mpsc::UnboundedSender<D2>,
) {
    match receiver.try_lock().ok() {
        Some(mut receiver) => match receiver.next().await {
            Some(Split::Hidari(data)) => {
                if hidari_sender.unbounded_send(data).is_err() {
                    warn!(target: "aleph-network", "Failed send despite controlling receiver, this shouldn't've happened.");
                }
            }
            Some(Split::Migi(data)) => {
                if migi_sender.unbounded_send(data).is_err() {
                    warn!(target: "aleph-network", "Failed send despite controlling receiver, this shouldn't've happened.");
                }
            }
            None => {
                hidari_sender.close_channel();
                migi_sender.close_channel();
            }
        },
        None => {
            interval(Duration::from_millis(MAXIMUM_RETRY_MS))
                .tick()
                .await;
        }
    }
}

const MAXIMUM_RETRY_MS: u64 = 500;

#[async_trait::async_trait]
impl<D1: Data, D2: Data, R: ReceiverComponent<Split<D1, D2>>> ReceiverComponent<D1>
    for HidariReceiver<D1, D2, R>
{
    async fn next(&mut self) -> Option<D1> {
        loop {
            tokio::select! {
                data = self.translated_receiver.next() => return data,
                _ = forward_or_wait(&self.receiver, &self.hidari_sender, &self.migi_sender) => (),
            }
        }
    }
}

#[async_trait::async_trait]
impl<D1: Data, D2: Data, R: ReceiverComponent<Split<D1, D2>>> ReceiverComponent<D2>
    for MigiReceiver<D1, D2, R>
{
    async fn next(&mut self) -> Option<D2> {
        loop {
            tokio::select! {
                data = self.translated_receiver.next() => return data,
                _ = forward_or_wait(&self.receiver, &self.hidari_sender, &self.migi_sender) => (),
            }
        }
    }
}

struct HidariNetwork<
    D1: Data,
    D2: Data,
    S: SenderComponent<Split<D1, D2>>,
    R: ReceiverComponent<Split<D1, D2>>,
> {
    sender: HidariSender<D1, D2, S>,
    receiver: Arc<Mutex<HidariReceiver<D1, D2, R>>>,
}

impl<
        D1: Data,
        D2: Data,
        S: SenderComponent<Split<D1, D2>>,
        R: ReceiverComponent<Split<D1, D2>>,
    > ComponentNetwork<D1> for HidariNetwork<D1, D2, S, R>
{
    type S = HidariSender<D1, D2, S>;
    type R = HidariReceiver<D1, D2, R>;
    fn sender(&self) -> &Self::S {
        &self.sender
    }
    fn receiver(&self) -> Arc<Mutex<Self::R>> {
        self.receiver.clone()
    }
}

struct MigiNetwork<
    D1: Data,
    D2: Data,
    S: SenderComponent<Split<D1, D2>>,
    R: ReceiverComponent<Split<D1, D2>>,
> {
    sender: MigiSender<D1, D2, S>,
    receiver: Arc<Mutex<MigiReceiver<D1, D2, R>>>,
}

impl<
        D1: Data,
        D2: Data,
        S: SenderComponent<Split<D1, D2>>,
        R: ReceiverComponent<Split<D1, D2>>,
    > ComponentNetwork<D2> for MigiNetwork<D1, D2, S, R>
{
    type S = MigiSender<D1, D2, S>;
    type R = MigiReceiver<D1, D2, R>;
    fn sender(&self) -> &Self::S {
        &self.sender
    }
    fn receiver(&self) -> Arc<Mutex<Self::R>> {
        self.receiver.clone()
    }
}

fn split_sender<D1: Data, D2: Data, S: SenderComponent<Split<D1, D2>>>(
    sender: &S,
) -> (HidariSender<D1, D2, S>, MigiSender<D1, D2, S>) {
    (
        HidariSender {
            sender: sender.clone(),
            d1: PhantomData,
            d2: PhantomData,
        },
        MigiSender {
            sender: sender.clone(),
            d1: PhantomData,
            d2: PhantomData,
        },
    )
}

fn split_receiver<D1: Data, D2: Data, R: ReceiverComponent<Split<D1, D2>>>(
    receiver: Arc<Mutex<R>>,
) -> (HidariReceiver<D1, D2, R>, MigiReceiver<D1, D2, R>) {
    let (hidari_sender, hidari_receiver) = mpsc::unbounded();
    let (migi_sender, migi_receiver) = mpsc::unbounded();
    (
        HidariReceiver {
            receiver: receiver.clone(),
            translated_receiver: hidari_receiver,
            hidari_sender: hidari_sender.clone(),
            migi_sender: migi_sender.clone(),
        },
        MigiReceiver {
            receiver,
            translated_receiver: migi_receiver,
            hidari_sender,
            migi_sender,
        },
    )
}

pub fn split<D1: Data, D2: Data, CN: ComponentNetwork<Split<D1, D2>>>(
    network: CN,
) -> (impl ComponentNetwork<D1>, impl ComponentNetwork<D2>) {
    let (hidari_sender, migi_sender) = split_sender(network.sender());
    let (hidari_receiver, migi_receiver) = split_receiver(network.receiver());
    (
        HidariNetwork {
            sender: hidari_sender,
            receiver: Arc::new(Mutex::new(hidari_receiver)),
        },
        MigiNetwork {
            sender: migi_sender,
            receiver: Arc::new(Mutex::new(migi_receiver)),
        },
    )
}
