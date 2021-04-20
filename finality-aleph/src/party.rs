use crate::{
    environment::Environment, network, network::ConsensusNetwork, AuthorityId, AuthorityKeystore,
    EpochId, NodeId, SpawnHandle,
};

use rush::{Config as ConsensusConfig, Consensus};
use sc_client_api::backend::Backend;
use sp_consensus::SelectChain;
use sp_core::{Blake2Hasher, Hasher, H256};
use sp_runtime::traits::Block;
use std::{marker::PhantomData, sync::Arc};

pub(crate) struct ConsensusParty<B, N, C, BE, SC>
where
    B: Block,
    N: network::Network<B>,
    C: crate::ClientForAleph<B, BE> + Send + Sync + 'static,
    BE: Backend<B> + 'static,
    SC: SelectChain<B> + 'static,
{
    network: ConsensusNetwork<B, H256, N>,
    client: Arc<C>,
    select_chain: SC,
    auth_keystore: AuthorityKeystore,
    //NOTE: not sure if this phantom is necessary here
    _phantom: std::marker::PhantomData<BE>,
}

impl<B, N: network::Network<B>, C, BE, SC> ConsensusParty<B, N, C, BE, SC>
where
    B: Block,
    N: network::Network<B> + 'static,
    C: crate::ClientForAleph<B, BE> + Send + Sync + 'static,
    BE: Backend<B> + 'static,
    SC: SelectChain<B> + 'static,
{
    pub(crate) fn new(
        client: Arc<C>,
        network: N,
        select_chain: SC,
        auth_keystore: AuthorityKeystore,
    ) -> Self {
        let network = ConsensusNetwork::new(network, "/cardinals/aleph/1");

        ConsensusParty {
            network,
            client,
            select_chain,
            auth_keystore,
            _phantom: PhantomData,
        }
    }

    pub(crate) fn run_network(&self, spawn_handle: SpawnHandle) {
        let mut network = self.network.clone();
        let task = async move {
            network.run().await;
        };
        rush::SpawnHandle::spawn(&spawn_handle, "aleph/network", task);
    }

    pub(crate) async fn run_epoch(
        &self,
        epoch_id: EpochId,
        authorities: Vec<AuthorityId>,
        conf: ConsensusConfig<NodeId>,
        spawn_handle: SpawnHandle,
    ) {
        let (tx_network, rx_network) = self.network.start_epoch(epoch_id, authorities.clone());
        let hashing = Blake2Hasher::hash;
        let auth_keystore = self.auth_keystore.clone();
        let client = self.client.clone();
        let select_chain = self.select_chain.clone();
        let mut env: Environment<B, H256, C, BE, SC> = Environment::new(
            client,
            select_chain,
            tx_network,
            rx_network,
            None,
            authorities,
            auth_keystore,
            hashing,
            epoch_id,
        );

        let (tx_out, rx_in, tx_order) = env.consensus_data();

        let consensus: Consensus<H256, NodeId> =
            Consensus::new(conf, rx_in, tx_out, tx_order, hashing);

        rush::SpawnHandle::spawn(&spawn_handle.clone(), "aleph/environment", env);
        log::debug!(target: "afa", "Environment has started");

        let (_exit, exit) = tokio::sync::oneshot::channel();
        log::debug!(target: "afa", "Consensus party has started");
        consensus.run(spawn_handle, exit).await;
        log::debug!(target: "afa", "Consensus party has stopped");
    }
}
