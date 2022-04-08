use anyhow::{ensure, Result};
use codec::{Decode, Encode};
use sp_core::{blake2_256, Pair};
use thiserror::Error;

use crate::{AccountId, KeyPair};

#[derive(Debug, Error)]
pub enum MultisigError {
    #[error("Threshold should be between 2 and {0}")]
    IncorrectThreshold(usize),
    #[error("There should be at least 2 unique members")]
    TooFewMembers,
}

/// `MultisigParty` is representing a multiparty entity constructed from
/// a group of accounts (`members`) and a threshold (`threshold`).
pub struct MultisigParty {
    /// Derived multiparty account (public key).
    account: AccountId,
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
        let account = Self::derive_multi_account(&accounts, threshold);
        Ok(Self {
            account,
            members: keypairs,
            threshold,
        })
    }

    /// This method generates deterministic account id for a given set of members and a threshold.
    ///
    /// It comes from pallet multisig. However, since it is the only thing we need from there,
    /// and it is really short, it makes sense to copy it instead adding new dependency.
    fn derive_multi_account(sorted_members: &[AccountId], threshold: u16) -> AccountId {
        let entropy = (b"modlpy/utilisuba", sorted_members, threshold).using_encoded(blake2_256);
        AccountId::decode(&mut &entropy[..]).unwrap_or_default()
    }
}
