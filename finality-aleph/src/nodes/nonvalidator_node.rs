use aleph_primitives::BlockNumber;
use log::{debug, error};
use sc_client_api::Backend;
use sc_network_common::ExHashT;
use sp_consensus::SelectChain;
use sp_runtime::traits::{Block, Header};

use crate::{
    session_map::{AuthorityProviderImpl, FinalityNotifierImpl, SessionMapUpdater},
    AlephConfig, BlockchainBackend,
    sync::Service as SyncService,
    finalization::AlephFinalizer,
    sync::{SubstrateFinalizationInfo, VerifierCache, SubstrateChainStatus, SubstrateChainStatusNotifier},
    network::{
        GossipService, SubstrateNetwork,
    },
};

pub async fn run_nonvalidator_node<B, H, C, BB, BE, SC>(aleph_config: AlephConfig<B, H, C, SC, BB>)
where
    B: Block,
    B::Header: Header<Number = BlockNumber>,
    H: ExHashT,
    C: crate::ClientForAleph<B, BE> + Send + Sync + 'static,
    C::Api: aleph_primitives::AlephSessionApi<B>,
    BE: Backend<B> + 'static,
    BB: BlockchainBackend<B> + Send + 'static,
    SC: SelectChain<B> + 'static,
{ }
