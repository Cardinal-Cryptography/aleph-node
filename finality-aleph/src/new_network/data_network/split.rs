use crate::{
    new_network::data_network::{
        aleph_network::{AlephNetwork, AlephNetworkData},
        rmc_network::{RmcNetwork, RmcNetworkData},
        Recipient, SessionCommand,
    },
    Error, NodeIndex, SessionId,
};
use codec::{Codec, Decode, Encode};
use futures::{channel::mpsc, Future, FutureExt, StreamExt};
use log::{debug, trace, warn};
use sp_api::BlockT;

#[derive(Clone, Encode, Decode, Debug)]
pub(crate) enum NetworkData<B: BlockT> {
    Aleph(AlephNetworkData<B>),
    Rmc(RmcNetworkData<B>),
}

pub(crate) struct DataNetwork<D: Clone + Codec> {
    session_id: SessionId,
    data_from_consensus_network: mpsc::UnboundedReceiver<D>,
    commands_for_consensus_network: mpsc::UnboundedSender<SessionCommand<D>>,
}

impl<D: Clone + Codec> DataNetwork<D> {
    fn new(
        session_id: SessionId,
        data_from_consensus_network: mpsc::UnboundedReceiver<D>,
        commands_for_consensus_network: mpsc::UnboundedSender<SessionCommand<D>>,
    ) -> Self {
        DataNetwork {
            session_id,
            data_from_consensus_network,
            commands_for_consensus_network,
        }
    }

    pub(crate) fn send(&self, data: D, recipient: Recipient<NodeIndex>) -> Result<(), Error> {
        let sc = SessionCommand::Data(self.session_id, data, recipient);
        self.commands_for_consensus_network
            .unbounded_send(sc)
            .map_err(|_| Error::SendData)
    }

    pub(crate) async fn next(&mut self) -> Option<D> {
        self.data_from_consensus_network.next().await
    }
}

pub(crate) fn split_network<B: BlockT>(
    data_network: DataNetwork<NetworkData<B>>,
    data_store_tx: mpsc::UnboundedSender<AlephNetworkData<B>>,
    data_store_rx: mpsc::UnboundedReceiver<AlephNetworkData<B>>,
) -> (AlephNetwork<B>, RmcNetwork<B>, impl Future<Output = ()>) {
    let (rmc_data_tx, rmc_data_rx) = mpsc::unbounded();
    let (aleph_cmd_tx, aleph_cmd_rx) = mpsc::unbounded();
    let (rmc_cmd_tx, rmc_cmd_rx) = mpsc::unbounded();
    let aleph_network = AlephNetwork::new(DataNetwork::new(
        data_network.session_id,
        data_store_rx,
        aleph_cmd_tx,
    ));
    let rmc_network = RmcNetwork::new(DataNetwork::new(
        data_network.session_id,
        rmc_data_rx,
        rmc_cmd_tx,
    ));
    let session_id = data_network.session_id;
    let mut data_from_consensus_network = data_network.data_from_consensus_network;
    let forward_data = async move {
        loop {
            match data_from_consensus_network.next().await {
                None => break,
                Some(NetworkData::Aleph(data)) => {
                    trace!(target: "afa", "Forwarding a message to DataStore {:?} {:?}", session_id, data);
                    if let Err(e) = data_store_tx.unbounded_send(data) {
                        debug!(target: "afa", "unable to send data for {:?} to DataStore {}", session_id, e);
                    }
                }
                Some(NetworkData::Rmc(data)) => {
                    trace!(target: "afa", "Forwarding a message to rmc {:?} {:?}", session_id, data);
                    if let Err(e) = rmc_data_tx.unbounded_send(data) {
                        debug!(target: "afa", "unable to send data for {:?} to rmc network {}", session_id, e);
                    }
                }
            }
        }
    };
    let cmd_tx = data_network.commands_for_consensus_network;
    let forward_aleph_cmd = {
        let cmd_tx = cmd_tx.clone();
        aleph_cmd_rx
            .map(|cmd| Ok(cmd.map(NetworkData::Aleph)))
            .forward(cmd_tx)
            .map(|res| {
                if let Err(e) = res {
                    warn!(target: "afa", "error forwarding aleph commands: {}", e);
                }
            })
    };
    let forward_rmc_cmd = {
        rmc_cmd_rx
            .map(|cmd| Ok(cmd.map(NetworkData::Rmc)))
            .forward(cmd_tx)
            .map(|res| {
                if let Err(e) = res {
                    warn!(target: "afa", "error forwarding rmc commands: {}", e);
                }
            })
    };
    let forwards = futures::future::join3(forward_data, forward_aleph_cmd, forward_rmc_cmd)
        .map(|((), (), ())| ());
    (aleph_network, rmc_network, forwards)
}
