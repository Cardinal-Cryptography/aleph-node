use crate::{justification::AlephJustification, AuthorityKeystore};
use aleph_primitives::ALEPH_ENGINE_ID;
use codec::Encode;
use log::{debug, error};
use sc_client_api::Backend;
use sp_api::{BlockId, NumberFor};
use sp_runtime::{traits::Block, Justification};
use std::sync::Arc;

pub(crate) fn finalize_block_as_authority<BE, B, C>(
    client: &Arc<C>,
    h: B::Hash,
    auth_keystore: &AuthorityKeystore,
) where
    B: Block,
    BE: Backend<B>,
    C: crate::ClientForAleph<B, BE>,
{
    let block_number = match client.number(h) {
        Ok(Some(number)) => number,
        _ => {
            error!(target: "afa", "a block with hash {} should already be in chain", h);
            return;
        }
    };
    finalize_block(
        client.clone(),
        h,
        block_number,
        Some((
            ALEPH_ENGINE_ID,
            AlephJustification::new::<B>(&auth_keystore, h).encode(),
        )),
    );
}

pub(crate) fn finalize_block<BE, B, C>(
    client: Arc<C>,
    hash: B::Hash,
    block_number: NumberFor<B>,
    justification: Option<Justification>,
) where
    B: Block,
    BE: Backend<B>,
    C: crate::ClientForAleph<B, BE>,
{
    let info = client.info();

    if info.finalized_number >= block_number {
        error!(target: "afa", "trying to finalized a block with hash {} and number {}
               that is not greater than already finalized {}", hash, block_number, info.finalized_number);
        return;
    }

    let status = client.info();
    debug!(target: "afa", "Finalizing block with hash {:?}. Previous best: #{:?}.", hash, status.finalized_number);

    let _update_res = client.lock_import_and_run(|import_op| {
        // NOTE: all other finalization logic should come here, inside the lock
        client.apply_finality(import_op, BlockId::Hash(hash), justification, true)
    });

    let status = client.info();
    debug!(target: "afa", "Finalized block with hash {:?}. Current best: #{:?}.", hash, status.finalized_number);
}
