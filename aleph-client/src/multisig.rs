use std::{collections::HashSet, str::FromStr};

use anyhow::{ensure, Result};
use codec::{Decode, Encode};
use log::error;
use pallet_multisig::Pallet;
use primitives::Balance;
use sp_core::{blake2_256, Pair};
use substrate_api_client::{compose_extrinsic, XtStatus};
use thiserror::Error;
use XtStatus::Finalized;

use crate::{try_send_xt, AccountId, BlockNumber, Connection, KeyPair, H256};

/// `MAX_WEIGHT` is the extrinsic parameter specifying upperbound for executing approved call.
/// Unless the approval is final, it has no effect. However, if due to your approval the
/// threshold is reached, you will be charged for execution process. By setting `max_weight`
/// low enough, you can avoid paying and left it for another member.
///
/// However, passing such parameter everytime is cumbersome and introduces the need of either
/// estimating call weight or setting very high universal bound at every caller side.
/// Thus, we keep a fairly high limit, which should cover almost any call (1 token).
const MAX_WEIGHT: u64 = 1;

/// Gathers all possible errors from this module.
#[derive(Debug, Error)]
pub enum MultisigError {
    #[error("❌ Threshold should be between 2 and {0}.")]
    IncorrectThreshold(usize),
    #[error("❌ There should be at least 2 unique members.")]
    TooFewMembers,
    #[error("❌ There is no available member at the provided index.")]
    IncorrectMemberIndex,
    #[error("❌ There is no entry for this multisig aggregation in the pallet storage.")]
    NoAggregationFound,
}

type CallHash = [u8; 32];
type Timepoint = pallet_multisig::Timepoint<BlockNumber>;

/// Unfortunately, we have to copy this struct from pallet. We can get such object from storage
/// but there is no way of accessing the info within nor interacting in any manner 💩.
#[derive(Clone, Decode)]
struct Multisig {
    when: Timepoint,
    deposit: Balance,
    depositor: AccountId,
    approvals: Vec<AccountId>,
}

/// This represents the ongoing procedure of aggregating approvals among members
/// of multisignature party.
#[derive(Clone, Debug)]
pub struct SignatureAggregation {
    /// The point in 'time' when the aggregation was initiated on the chain.
    /// Internally it is a pair: number of the block containing initial call and the position
    /// of the corresponding extrinsic within block.
    ///
    /// It is actually the easiest (and the chosen) way of distinguishing between
    /// independent aggregations within the same party for the same call.
    timepoint: Timepoint,
    /// The member, who initiated the aggregation. They also had to deposit money, and they
    /// are the only person with power of canceling the procedure.
    ///
    /// We keep just their index within the (sorted) set of members.
    author: usize,
    /// The hash of the target call.
    call_hash: CallHash,
    /// The call itself. Maybe.
    call: Option<String>,
    /// We keep counting approvals, just for information.
    approvers: HashSet<AccountId>,
}

/// `MultisigParty` is representing a multiparty entity constructed from
/// a group of accounts (`members`) and a threshold (`threshold`).
pub struct MultisigParty {
    /// Derived multiparty account (public key).
    pub account: AccountId,
    /// *Sorted* collection of members.
    members: Vec<KeyPair>,
    /// Minimum required approvals.
    threshold: u16,
}

impl MultisigParty {
    /// Creates new party. `members` does *not* have to be already sorted. Also:
    /// - `members` must be of length between 2 and `pallet_multisig::MaxSignatories`;
    ///    since checking the upperbound is expensive, it is not the caller's responsibility
    ///    to ensure it is not exceeded
    /// - `members` may contain duplicates, but they are ignored and not counted to the cardinality
    /// - `threshold` must be between 2 and `members.len()`
    pub fn new(members: &[KeyPair], threshold: u16) -> Result<Self> {
        let mut members = members
            .iter()
            .map(|m| (m.clone(), AccountId::from(m.public())))
            .collect::<Vec<_>>();

        members.sort_by_key(|(_, a)| a.clone());
        members.dedup_by(|(_, a1), (_, a2)| a1 == a2);

        ensure!(2 <= members.len(), MultisigError::TooFewMembers);
        ensure!(
            2 <= threshold && threshold <= members.len() as u16,
            MultisigError::IncorrectThreshold(members.len())
        );

        let (keypairs, accounts): (Vec<_>, Vec<_>) = members.iter().cloned().unzip();
        let account = Self::multi_account_id(&accounts, threshold);
        Ok(Self {
            account,
            members: keypairs,
            threshold,
        })
    }

    pub fn multi_account_id(who: &[AccountId], threshold: u16) -> AccountId {
        let entropy = (b"modlpy/utilisuba", who, threshold).using_encoded(blake2_256);
        AccountId::decode(&mut &entropy[..]).unwrap_or_default()
    }

    /// Effectively calls `approveAsMulti`.
    pub fn initiate_aggregation_with_hash(
        &self,
        connection: &Connection,
        call_hash: CallHash,
        author_idx: usize,
    ) -> Result<SignatureAggregation> {
        ensure!(
            author_idx < self.members.len(),
            MultisigError::IncorrectMemberIndex
        );

        let (author, other_signatories) = self.designate_representative_and_represented(author_idx);
        error!("{:?}", author.public());
        let connection = connection.clone().set_signer(author.clone());
        let chuj: Option<String> = None;
        let xt = compose_extrinsic!(
            &connection,
            "Multisig",
            "approve_as_multi",
            self.threshold,
            other_signatories,
            chuj,
            call_hash.clone(),
            MAX_WEIGHT
        );
        error!("{:?}", xt.function.encode());
        let block_hash = try_send_xt(
            &connection,
            xt,
            Some("Initiate multisig aggregation"),
            Finalized,
        )?
        .expect("For `Finalized` status a block hash should be returned");

        Ok(SignatureAggregation {
            timepoint: self.get_timestamp(&connection, call_hash.clone(), block_hash)?,
            author: author_idx,
            call_hash,
            call: None,
            approvers: HashSet::from([AccountId::from(author.public())]),
        })
    }

    /// For all extrinsics we have to sign it with the caller (representative) and pass
    /// accounts of the other party members.
    fn designate_representative_and_represented(&self, idx: usize) -> (KeyPair, Vec<AccountId>) {
        let mut members = self.members.clone();
        let member = members.remove(idx);
        let others = members
            .iter()
            .map(|m| AccountId::from(m.public()))
            .collect();
        (member, others)
    }

    /// Reads the pallet storage and takes the timestamp regarding procedure for the `self` party
    /// initiated at `block_hash`.
    fn get_timestamp(
        &self,
        connection: &Connection,
        call_hash: CallHash,
        block_hash: H256,
    ) -> Result<Timepoint> {
        error!("block hash: {:?}", block_hash);
        error!("account: {:?}", self.account);

        let chuj: Option<H256> = None;
        let multisig: Multisig = connection
            .get_storage_double_map(
                "Multisig",
                "Multisigs",
                self.account.clone(),
                call_hash,
                Some(block_hash),
            )?
            .ok_or(MultisigError::NoAggregationFound)?;
        Ok(multisig.when)
    }
}
