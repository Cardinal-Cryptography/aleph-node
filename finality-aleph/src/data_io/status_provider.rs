use log::{debug, warn};
use sp_runtime::SaturatedConversion;

use crate::{
    aleph_primitives::BlockNumber,
    block::{Header, HeaderVerifier, UnverifiedHeader},
    data_io::{
        chain_info::ChainInfoProvider,
        proposal::{AlephProposal, PendingProposalStatus, ProposalStatus},
    },
};

pub fn get_proposal_status<CIP, H, V>(
    chain_info_provider: &mut CIP,
    header_verifier: &mut V,
    proposal: &AlephProposal<H::Unverified>,
    old_status: Option<&ProposalStatus>,
) -> ProposalStatus
where
    CIP: ChainInfoProvider,
    H: Header,
    V: HeaderVerifier<H>,
{
    use PendingProposalStatus::*;
    use ProposalStatus::*;

    let current_highest_finalized = chain_info_provider.get_highest_finalized().number();

    if current_highest_finalized >= proposal.number_top_block() {
        return Ignore;
    }

    if is_hopeless_fork(chain_info_provider, proposal) {
        debug!(target: "aleph-finality", "Encountered a hopeless fork proposal {:?}.", proposal);
        return Ignore;
    }

    let old_status = match old_status {
        Some(status) => status,
        None => {
            // Verify header here, so it happens at most once. Incorrect headers are equivalent to a broken branch,
            // since we cannot depend on the blocks represented by them being ever acquired.
            match header_verifier.verify_header(proposal.top_block_header(), false) {
                Ok(_) => &Pending(PendingTopBlock),
                Err(e) => {
                    warn!(target: "aleph-finality", "Invalid header in proposal: {}", e);
                    &Pending(TopBlockImportedButIncorrectBranch)
                }
            }
        }
    };
    match old_status {
        Pending(PendingTopBlock) => {
            let top_block = proposal.top_block();
            if chain_info_provider.is_block_imported(&top_block) {
                // Note that the above also makes sure that the `number` claimed in the proposal is correct.
                // That's why checking the branch correctness now boils down to checking the parent-child
                // relation on the branch.
                if is_branch_ancestry_correct(chain_info_provider, proposal) {
                    if is_ancestor_finalized(chain_info_provider, proposal) {
                        Finalize(
                            proposal
                                .blocks_from_num(current_highest_finalized + 1)
                                .collect(),
                        )
                    } else {
                        // This could also be a hopeless fork, but we have checked before that it isn't (yet).
                        Pending(TopBlockImportedButNotFinalizedAncestor)
                    }
                } else {
                    // This could also be a hopeless fork, but we have checked before that it isn't (yet).
                    Pending(TopBlockImportedButIncorrectBranch)
                }
            } else {
                // This could also be a hopeless fork, but we have checked before that it isn't (yet).
                Pending(PendingTopBlock)
            }
        }
        Pending(TopBlockImportedButNotFinalizedAncestor) => {
            if is_ancestor_finalized(chain_info_provider, proposal) {
                Finalize(
                    proposal
                        .blocks_from_num(current_highest_finalized + 1)
                        .collect(),
                )
            } else {
                // This could also be a hopeless fork, but we have checked before that it isn't (yet).
                Pending(TopBlockImportedButNotFinalizedAncestor)
            }
        }
        Pending(TopBlockImportedButIncorrectBranch) => {
            // This could also be a hopeless fork, but we have checked before that it isn't (yet).
            Pending(TopBlockImportedButIncorrectBranch)
        }
        _ => old_status.clone(),
    }
}

fn is_hopeless_fork<CIP, UH>(chain_info_provider: &mut CIP, proposal: &AlephProposal<UH>) -> bool
where
    CIP: ChainInfoProvider,
    UH: UnverifiedHeader,
{
    let bottom_num = proposal.number_bottom_block();
    for (i, id) in proposal.blocks_from_num(bottom_num).enumerate() {
        if let Ok(finalized_block) =
            chain_info_provider.get_finalized_at(bottom_num + <BlockNumber>::saturated_from(i))
        {
            if finalized_block.hash() != id.hash() {
                return true;
            }
        } else {
            // We don't know the finalized block at this height
            break;
        }
    }
    false
}

fn is_ancestor_finalized<CIP, UH>(
    chain_info_provider: &mut CIP,
    proposal: &AlephProposal<UH>,
) -> bool
where
    CIP: ChainInfoProvider,
    UH: UnverifiedHeader,
{
    let bottom = proposal.bottom_block();
    let parent_hash = if let Ok(hash) = chain_info_provider.get_parent_hash(&bottom) {
        hash
    } else {
        return false;
    };
    let finalized =
        if let Ok(hash) = chain_info_provider.get_finalized_at(proposal.number_below_branch()) {
            hash
        } else {
            return false;
        };
    parent_hash == finalized.hash()
}

// Checks that the subsequent blocks in the branch are in the parent-child relation, as required.
fn is_branch_ancestry_correct<CIP, UH>(
    chain_info_provider: &mut CIP,
    proposal: &AlephProposal<UH>,
) -> bool
where
    CIP: ChainInfoProvider,
    UH: UnverifiedHeader,
{
    let bottom_num = proposal.number_bottom_block();
    for (parent, current) in proposal
        .blocks_from_num(bottom_num)
        .zip(proposal.blocks_from_num(bottom_num + 1))
    {
        match chain_info_provider.get_parent_hash(&current) {
            Ok(parent_hash) => {
                if parent_hash != parent.hash() {
                    return false;
                }
            }
            Err(()) => {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use std::{num::NonZeroUsize, sync::Arc};

    use sp_runtime::traits::Block as BlockT;

    use crate::{
        data_io::{
            chain_info::{
                AuxFinalizationChainInfoProvider, CachedChainInfoProvider,
                SubstrateChainInfoProvider,
            },
            proposal::{
                AlephProposal,
                PendingProposalStatus::*,
                ProposalStatus::{self, *},
            },
            status_provider::get_proposal_status,
            ChainInfoCacheConfig, MAX_DATA_BRANCH_LEN,
        },
        testing::{
            client_chain_builder::ClientChainBuilder,
            mocks::{
                unvalidated_proposal_from_headers, TBlock, THeader, TestClient, TestClientBuilder,
                TestClientBuilderExt, TestVerifier,
            },
        },
        SessionBoundaryInfo, SessionId, SessionPeriod,
    };

    // A large number only for the purpose of creating `AlephProposal`s
    const DUMMY_SESSION_LEN: u32 = 1_000_000;

    fn proposal_from_headers(headers: Vec<THeader>) -> AlephProposal<THeader> {
        let unvalidated = unvalidated_proposal_from_headers(headers);
        let session_boundaries = SessionBoundaryInfo::new(SessionPeriod(DUMMY_SESSION_LEN))
            .boundaries_for_session(SessionId(0));
        unvalidated.validate_bounds(&session_boundaries).unwrap()
    }

    fn proposal_from_blocks(blocks: Vec<TBlock>) -> AlephProposal<THeader> {
        let headers = blocks.into_iter().map(|b| b.header().clone()).collect();
        proposal_from_headers(headers)
    }

    type TestCachedChainInfo =
        CachedChainInfoProvider<SubstrateChainInfoProvider<THeader, Arc<TestClient>>>;
    type TestAuxChainInfo =
        AuxFinalizationChainInfoProvider<SubstrateChainInfoProvider<THeader, Arc<TestClient>>>;

    fn prepare_proposal_test() -> (ClientChainBuilder, TestCachedChainInfo, TestAuxChainInfo) {
        let client = Arc::new(TestClientBuilder::new().build());

        let config = ChainInfoCacheConfig {
            block_cache_capacity: NonZeroUsize::new(2).unwrap(),
        };
        let cached_chain_info_provider =
            CachedChainInfoProvider::new(SubstrateChainInfoProvider::new(client.clone()), config);

        let chain_builder =
            ClientChainBuilder::new(client.clone(), Arc::new(TestClientBuilder::new().build()));

        let aux_chain_info_provider = AuxFinalizationChainInfoProvider::new(
            SubstrateChainInfoProvider::new(client),
            chain_builder.genesis_id(),
        );

        (
            chain_builder,
            cached_chain_info_provider,
            aux_chain_info_provider,
        )
    }

    fn verify_proposal_status(
        cached_cip: &mut TestCachedChainInfo,
        aux_cip: &mut TestAuxChainInfo,
        proposal: &AlephProposal<THeader>,
        correct_status: ProposalStatus,
    ) {
        let status_a = get_proposal_status(aux_cip, &mut TestVerifier, proposal, None);
        assert_eq!(
            status_a, correct_status,
            "Aux chain info gives wrong status for proposal {proposal:?}"
        );
        let status_c = get_proposal_status(cached_cip, &mut TestVerifier, proposal, None);
        assert_eq!(
            status_c, correct_status,
            "Cached chain info gives wrong status for proposal {proposal:?}"
        );
    }

    fn verify_proposal_of_all_lens_finalizable(
        blocks: Vec<TBlock>,
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
                ProposalStatus::Finalize(proposal.blocks_from_num(0).collect()),
            );
        }
    }

    #[tokio::test]
    async fn correct_proposals_are_finalizable_even_with_forks() {
        let (mut chain_builder, mut cached_cip, mut aux_cip) = prepare_proposal_test();
        let blocks = chain_builder
            .initialize_single_branch_and_import(MAX_DATA_BRANCH_LEN * 10)
            .await;

        verify_proposal_of_all_lens_finalizable(blocks.clone(), &mut cached_cip, &mut aux_cip);

        let _fork = chain_builder
            .build_and_import_branch_above(&blocks[2].header.hash(), MAX_DATA_BRANCH_LEN * 10)
            .await;

        verify_proposal_of_all_lens_finalizable(blocks.clone(), &mut cached_cip, &mut aux_cip);
    }

    #[tokio::test]
    async fn not_finalized_ancestors_handled_correctly() {
        let (mut chain_builder, mut cached_cip, mut aux_cip) = prepare_proposal_test();
        let blocks = chain_builder
            .initialize_single_branch_and_import(MAX_DATA_BRANCH_LEN * 10)
            .await;

        let fork = chain_builder
            .build_and_import_branch_above(&blocks[2].header.hash(), MAX_DATA_BRANCH_LEN * 10)
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
        let blocks = chain_builder
            .initialize_single_branch_and_import(MAX_DATA_BRANCH_LEN * 10)
            .await;

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
        verify_proposal_status(&mut cached_cip, &mut aux_cip, &proposal, Ignore);
    }

    #[tokio::test]
    async fn pending_top_block_handled_correctly() {
        let (mut chain_builder, mut cached_cip, mut aux_cip) = prepare_proposal_test();
        let blocks = chain_builder
            .initialize_single_branch(MAX_DATA_BRANCH_LEN * 10)
            .await;

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
        let blocks = chain_builder
            .initialize_single_branch_and_import(MAX_DATA_BRANCH_LEN * 10)
            .await;

        let fork = chain_builder
            .build_branch_above(&blocks[2].header.hash(), MAX_DATA_BRANCH_LEN * 10)
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
            verify_proposal_status(&mut cached_cip, &mut aux_cip, &proposal, Ignore);
        }
        // Proposal below finalized should be ignored
        verify_proposal_status(
            &mut cached_cip,
            &mut aux_cip,
            &proposal_from_blocks(blocks[0..4].to_vec()),
            Ignore,
        );

        // New proposals above finalized should be finalizable.
        let fresh_proposal = proposal_from_blocks(blocks[4..6].to_vec());
        verify_proposal_status(
            &mut cached_cip,
            &mut aux_cip,
            &fresh_proposal,
            Finalize(fresh_proposal.blocks_from_num(0).collect()),
        );

        // Long proposals should finalize the appropriate suffix.
        let long_proposal = proposal_from_blocks(blocks[0..6].to_vec());
        verify_proposal_status(
            &mut cached_cip,
            &mut aux_cip,
            &long_proposal,
            // We are using fresh_proposal here on purpose, to only check the expected blocks.
            Finalize(fresh_proposal.blocks_from_num(0).collect()),
        );
    }
}
