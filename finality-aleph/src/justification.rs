use crate::{network, AuthorityKeystore, JustificationNotification, KeyBox, Signature};
use aleph_bft::{MultiKeychain, NodeIndex, SignatureSet};
use aleph_primitives::{AuthorityId, Session};
use codec::{Decode, Encode};
use futures::channel::mpsc;
use parking_lot::Mutex;
use sp_api::{BlockT, NumberFor};
use sp_blockchain::Error;
use std::{collections::HashMap, sync::Arc};
use tokio::stream::StreamExt;

#[derive(Clone, Encode, Decode, Debug)]
pub struct AlephJustification {
    pub(crate) signature: SignatureSet<Signature>,
}

impl AlephJustification {
    pub(crate) fn new<Block: BlockT>(signature: SignatureSet<Signature>) -> Self {
        Self { signature }
    }

    pub(crate) fn decode_and_verify<
        Block: BlockT,
        MK: MultiKeychain<Signature = Signature, PartialMultisignature = SignatureSet<Signature>>,
    >(
        justification: &[u8],
        block_hash: Block::Hash,
        multi_keychain: &MK,
    ) -> Result<AlephJustification, Error> {
        let aleph_justification = AlephJustification::decode(&mut &*justification)
            .map_err(|_| Error::JustificationDecode)?;

        let valid =
            multi_keychain.is_complete(&block_hash.encode()[..], &aleph_justification.signature);

        if !valid {
            log::debug!(target: "afa", "Bad justification decoded for block hash #{:?}", block_hash);
            return Err(Error::BadJustification("Invalid justification".into()));
        }

        Ok(aleph_justification)
    }
}

type SessionMap<Block> = HashMap<u32, Session<AuthorityId, NumberFor<Block>>>;

pub struct JustificationHandler<Block: BlockT, N: network::Network<Block> + 'static> {
    finalization_proposals_tx: mpsc::UnboundedSender<JustificationNotification<Block>>,
    justification_rx: mpsc::UnboundedReceiver<JustificationNotification<Block>>,
    sessions: Arc<Mutex<SessionMap<Block>>>,
    auth_keystore: AuthorityKeystore,
    session_period: u32,
    network: N,
}

impl<Block: BlockT, N: network::Network<Block> + 'static> JustificationHandler<Block, N>
where
    NumberFor<Block>: Into<u32>,
{
    pub(crate) fn new(
        finalization_proposals_tx: mpsc::UnboundedSender<JustificationNotification<Block>>,
        justification_rx: mpsc::UnboundedReceiver<JustificationNotification<Block>>,
        sessions: Arc<Mutex<SessionMap<Block>>>,
        auth_keystore: AuthorityKeystore,
        session_period: u32,
        network: N,
    ) -> Self {
        Self {
            finalization_proposals_tx,
            justification_rx,
            sessions,
            auth_keystore,
            session_period,
            network,
        }
    }

    pub(crate) async fn run(mut self) {
        while let Some(notification) = self.justification_rx.next().await {
            let keybox = match self.session_keybox(notification.number) {
                Some(keybox) => keybox,
                None => {
                    self.network
                        .request_justification(&notification.hash, notification.number);
                    continue;
                }
            };

            match AlephJustification::decode_and_verify::<Block, _>(
                &notification.justification,
                notification.hash,
                &crate::MultiKeychain::new(keybox),
            ) {
                Ok(_) => self
                    .finalization_proposals_tx
                    .unbounded_send(notification)
                    .expect("Notification should succeed"),
                Err(err) => {
                    log::error!(target: "afa", "{:?}", err);
                    self.network
                        .request_justification(&notification.hash, notification.number);
                }
            }
        }

        log::error!(target: "afa", "Notification channel closed unexpectedly");
    }

    fn session_keybox(&self, n: NumberFor<Block>) -> Option<KeyBox> {
        let session = n.into() / self.session_period;
        let authorities = match self.sessions.lock().get(&session) {
            Some(session) => session.authorities.to_vec(),
            None => return None,
        };

        Some(KeyBox {
            authorities,
            auth_keystore: self.auth_keystore.clone(),
            id: NodeIndex(0),
        })
    }
}
