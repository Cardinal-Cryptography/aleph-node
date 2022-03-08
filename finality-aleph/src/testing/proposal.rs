use crate::{
    data_io::{
        get_proposal_status, AlephProposal, AuxFinalizationChainInfoProvider,
        CachedChainInfoProvider, ChainInfoCacheConfig,
        IgnoredProposalReason::*,
        PendingProposalStatus::*,
        ProposalStatus::{self, *},
        UnvalidatedAlephProposal, MAX_DATA_BRANCH_LEN,
    },
    testing::client_chain_builder::ClientChainBuilder,
    SessionBoundaries, SessionId, SessionPeriod,
};

use sp_runtime::traits::Block as BlockT;
use std::sync::Arc;
use substrate_test_runtime_client::{
    runtime::{Block, Header},
    DefaultTestClientBuilderExt, TestClient, TestClientBuilder, TestClientBuilderExt,
};

fn proposal_from_headers(headers: Vec<Header>) -> AlephProposal<Block> {
    let num = headers.last().unwrap().number;
    let hashes = headers.into_iter().map(|header| header.hash()).collect();
    let unvalidated = UnvalidatedAlephProposal::new(hashes, num);
    let session_boundaries = SessionBoundaries::new(SessionId(0), SessionPeriod(10000));
    unvalidated.validate_bounds(&session_boundaries).unwrap()
}

fn proposal_from_blocks(blocks: Vec<Block>) -> AlephProposal<Block> {
    let headers = blocks.into_iter().map(|b| b.header().clone()).collect();
    proposal_from_headers(headers)
}

type TestCachedChainInfo = CachedChainInfoProvider<Block, Arc<TestClient>>;
type TestAuxChainInfo = AuxFinalizationChainInfoProvider<Block, Arc<TestClient>>;

fn prepare_proposal_test() -> (ClientChainBuilder, TestCachedChainInfo, TestAuxChainInfo) {
    let client = Arc::new(TestClientBuilder::new().build());

    let config = ChainInfoCacheConfig {
        block_cache_capacity: 2,
    };
    let cached_chain_info_provider = CachedChainInfoProvider::new(client.clone(), config);

    let chain_builder =
        ClientChainBuilder::new(client.clone(), Arc::new(TestClientBuilder::new().build()));

    let aux_chain_info_provider =
        AuxFinalizationChainInfoProvider::new(client, chain_builder.genesis_hash_num());

    (
        chain_builder,
        cached_chain_info_provider,
        aux_chain_info_provider,
    )
}

async fn initialize_single_branch(chain_builder: &mut ClientChainBuilder) -> Vec<Block> {
    chain_builder
        .build_branch_upon(&chain_builder.genesis_hash(), MAX_DATA_BRANCH_LEN * 10)
        .await
}

async fn initialize_single_branch_and_import(chain_builder: &mut ClientChainBuilder) -> Vec<Block> {
    chain_builder
        .build_and_import_branch_upon(&chain_builder.genesis_hash(), MAX_DATA_BRANCH_LEN * 10)
        .await
}

fn verify_proposal_status(
    cached_cip: &mut TestCachedChainInfo,
    aux_cip: &mut TestAuxChainInfo,
    proposal: &AlephProposal<Block>,
    correct_status: ProposalStatus<Block>,
) {
    let status_a = get_proposal_status(aux_cip, proposal, None);
    assert_eq!(
        status_a, correct_status,
        "Aux chain info gives wrong status for proposal {:?}",
        proposal
    );
    let status_c = get_proposal_status(cached_cip, proposal, None);
    assert_eq!(
        status_c, correct_status,
        "Cached chain info gives wrong status for proposal {:?}",
        proposal
    );
}

fn verify_proposal_of_all_lens_finalizable(
    blocks: Vec<Block>,
    cached_cip: &mut TestCachedChainInfo,
    aux_cip: &mut TestAuxChainInfo,
) {
    for len in 1..=MAX_DATA_BRANCH_LEN {
        let blocks_branch = blocks[0..len].to_vec();
        let proposal = proposal_from_blocks(blocks_branch);
        verify_proposal_status(
            cached_cip,
            aux_cip,
            &proposal,
            ProposalStatus::Finalize(proposal.top_block()),
        );
    }
}

#[tokio::test]
async fn correct_proposals_are_finalizable_even_with_forks() {
    let (mut chain_builder, mut cached_cip, mut aux_cip) = prepare_proposal_test();
    let blocks = initialize_single_branch_and_import(&mut chain_builder).await;

    verify_proposal_of_all_lens_finalizable(blocks.clone(), &mut cached_cip, &mut aux_cip);

    let _fork = chain_builder
        .build_and_import_branch_upon(&blocks[2].header.hash(), MAX_DATA_BRANCH_LEN * 10)
        .await;

    verify_proposal_of_all_lens_finalizable(blocks.clone(), &mut cached_cip, &mut aux_cip);
}

#[tokio::test]
async fn not_finalized_ancestors_handled_correctly() {
    let (mut chain_builder, mut cached_cip, mut aux_cip) = prepare_proposal_test();
    let blocks = initialize_single_branch_and_import(&mut chain_builder).await;

    let fork = chain_builder
        .build_and_import_branch_upon(&blocks[2].header.hash(), MAX_DATA_BRANCH_LEN * 10)
        .await;

    for len in 1..=MAX_DATA_BRANCH_LEN {
        let blocks_branch = blocks[1..(len + 1)].to_vec();
        let proposal = proposal_from_blocks(blocks_branch);
        verify_proposal_status(
            &mut cached_cip,
            &mut aux_cip,
            &proposal,
            Pending(TopBlockImportedButNotFinalizedAncestor),
        );
        let blocks_branch = fork[1..(len + 1)].to_vec();
        let proposal = proposal_from_blocks(blocks_branch);
        verify_proposal_status(
            &mut cached_cip,
            &mut aux_cip,
            &proposal,
            Pending(TopBlockImportedButNotFinalizedAncestor),
        );
    }
}

#[tokio::test]
async fn incorrect_branch_handled_correctly() {
    let (mut chain_builder, mut cached_cip, mut aux_cip) = prepare_proposal_test();
    let blocks = initialize_single_branch_and_import(&mut chain_builder).await;

    let incorrect_branch = vec![
        blocks[0].clone(),
        blocks[1].clone(),
        blocks[3].clone(),
        blocks[5].clone(),
    ];
    let proposal = proposal_from_blocks(incorrect_branch);
    verify_proposal_status(
        &mut cached_cip,
        &mut aux_cip,
        &proposal,
        Pending(TopBlockImportedButIncorrectBranch),
    );

    chain_builder.finalize_block(&blocks[1].header.hash());
    verify_proposal_status(
        &mut cached_cip,
        &mut aux_cip,
        &proposal,
        Pending(TopBlockImportedButIncorrectBranch),
    );

    chain_builder.finalize_block(&blocks[10].header.hash());
    verify_proposal_status(&mut cached_cip, &mut aux_cip, &proposal, Ignore(TooLow));
}

#[tokio::test]
async fn pending_top_block_handled_correctly() {
    let (mut chain_builder, mut cached_cip, mut aux_cip) = prepare_proposal_test();
    let blocks = initialize_single_branch(&mut chain_builder).await;

    for len in 1..=MAX_DATA_BRANCH_LEN {
        let blocks_branch = blocks[0..len].to_vec();
        let proposal = proposal_from_blocks(blocks_branch);
        verify_proposal_status(
            &mut cached_cip,
            &mut aux_cip,
            &proposal,
            Pending(PendingTopBlock),
        );
    }
    chain_builder.import_branch(blocks.clone()).await;

    verify_proposal_of_all_lens_finalizable(blocks, &mut cached_cip, &mut aux_cip);
}

#[tokio::test]
async fn hopeless_forks_handled_correctly() {
    let (mut chain_builder, mut cached_cip, mut aux_cip) = prepare_proposal_test();
    let blocks = initialize_single_branch_and_import(&mut chain_builder).await;

    let fork = chain_builder
        .build_branch_upon(&blocks[2].header.hash(), MAX_DATA_BRANCH_LEN * 10)
        .await;

    for len in 1..=MAX_DATA_BRANCH_LEN {
        let fork_branch = fork[0..len].to_vec();
        let proposal = proposal_from_blocks(fork_branch);
        verify_proposal_status(
            &mut cached_cip,
            &mut aux_cip,
            &proposal,
            Pending(PendingTopBlock),
        );
    }

    chain_builder.finalize_block(&blocks[2].header.hash());

    for len in 1..=MAX_DATA_BRANCH_LEN {
        let fork_branch = fork[0..len].to_vec();
        let proposal = proposal_from_blocks(fork_branch);
        verify_proposal_status(
            &mut cached_cip,
            &mut aux_cip,
            &proposal,
            Pending(PendingTopBlock),
        );
    }

    chain_builder.finalize_block(&blocks[3].header.hash());

    for len in 1..=MAX_DATA_BRANCH_LEN {
        let fork_branch = fork[0..len].to_vec();
        let proposal = proposal_from_blocks(fork_branch);
        let expected_reason = if len == 1 { TooLow } else { HopelessFork };
        verify_proposal_status(
            &mut cached_cip,
            &mut aux_cip,
            &proposal,
            Ignore(expected_reason),
        );
    }
    // Proposal below finalized should be ignored
    verify_proposal_status(
        &mut cached_cip,
        &mut aux_cip,
        &proposal_from_blocks(blocks[0..4].to_vec()),
        Ignore(TooLow),
    );

    // New proposals above finalized should be finalizable.
    let fresh_proposal = proposal_from_blocks(blocks[4..6].to_vec());
    verify_proposal_status(
        &mut cached_cip,
        &mut aux_cip,
        &fresh_proposal,
        Finalize(fresh_proposal.top_block()),
    );
}
