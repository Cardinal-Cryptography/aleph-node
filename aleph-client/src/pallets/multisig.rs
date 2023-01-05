use std::collections::HashSet;

use anyhow::{anyhow, ensure, Result as AnyResult};
use codec::{Decode, Encode};
use primitives::{Balance, BlockNumber};
use sp_core::blake2_256;
use sp_runtime::traits::TrailingZeroInput;

use crate::{
    account_from_keypair, aleph_runtime::RuntimeCall, api, api::runtime_types,
    sp_weights::weight_v2::Weight, AccountId, BlockHash, ConnectionApi, SignedConnectionApi,
    TxStatus,
};

pub type CallHash = [u8; 32];
pub type Call = RuntimeCall;
pub type MultisigThreshold = u16;
pub type Timepoint = runtime_types::pallet_multisig::Timepoint<BlockNumber>;
pub type Multisig = runtime_types::pallet_multisig::Multisig<BlockNumber, Balance, AccountId>;

pub const DEFAULT_MAX_WEIGHT: Weight = Weight::new(500_000_000, 0);

#[async_trait::async_trait]
pub trait MultisigUserApi {
    async fn as_multi_threshold_1(
        &self,
        other_signatories: Vec<AccountId>,
        call: Call,
        status: TxStatus,
    ) -> anyhow::Result<BlockHash>;
    async fn as_multi(
        &self,
        threshold: MultisigThreshold,
        other_signatories: Vec<AccountId>,
        timepoint: Option<Timepoint>,
        max_weight: Weight,
        call: Call,
        status: TxStatus,
    ) -> anyhow::Result<BlockHash>;
    async fn approve_as_multi(
        &self,
        threshold: MultisigThreshold,
        other_signatories: Vec<AccountId>,
        timepoint: Option<Timepoint>,
        max_weight: Weight,
        call_hash: CallHash,
        status: TxStatus,
    ) -> anyhow::Result<BlockHash>;
    async fn cancel_as_multi(
        &self,
        threshold: MultisigThreshold,
        other_signatories: Vec<AccountId>,
        timepoint: Timepoint,
        call_hash: CallHash,
        status: TxStatus,
    ) -> anyhow::Result<BlockHash>;
}

#[async_trait::async_trait]
impl<S: SignedConnectionApi> MultisigUserApi for S {
    async fn as_multi_threshold_1(
        &self,
        other_signatories: Vec<AccountId>,
        call: Call,
        status: TxStatus,
    ) -> anyhow::Result<BlockHash> {
        let tx = api::tx()
            .multisig()
            .as_multi_threshold_1(other_signatories, call);

        self.send_tx(tx, status).await
    }

    async fn as_multi(
        &self,
        threshold: MultisigThreshold,
        other_signatories: Vec<AccountId>,
        timepoint: Option<Timepoint>,
        max_weight: Weight,
        call: Call,
        status: TxStatus,
    ) -> anyhow::Result<BlockHash> {
        let tx = api::tx().multisig().as_multi(
            threshold,
            other_signatories,
            timepoint,
            call,
            max_weight,
        );

        self.send_tx(tx, status).await
    }

    async fn approve_as_multi(
        &self,
        threshold: MultisigThreshold,
        other_signatories: Vec<AccountId>,
        timepoint: Option<Timepoint>,
        max_weight: Weight,
        call_hash: CallHash,
        status: TxStatus,
    ) -> anyhow::Result<BlockHash> {
        let tx = api::tx().multisig().approve_as_multi(
            threshold,
            other_signatories,
            timepoint,
            call_hash,
            max_weight,
        );

        self.send_tx(tx, status).await
    }

    async fn cancel_as_multi(
        &self,
        threshold: MultisigThreshold,
        other_signatories: Vec<AccountId>,
        timepoint: Timepoint,
        call_hash: CallHash,
        status: TxStatus,
    ) -> anyhow::Result<BlockHash> {
        let tx = api::tx().multisig().cancel_as_multi(
            threshold,
            other_signatories,
            timepoint,
            call_hash,
        );

        self.send_tx(tx, status).await
    }
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct MultisigParty {
    signatories: Vec<AccountId>,
    threshold: MultisigThreshold,
}

impl MultisigParty {
    // no upperbound check
    pub fn new(signatories: &[AccountId], threshold: MultisigThreshold) -> AnyResult<Self> {
        let mut sorted_signatories = signatories.to_vec();
        sorted_signatories.sort();
        sorted_signatories.dedup();

        ensure!(
            sorted_signatories.len() > 1,
            "There must be at least 2 different signatories"
        );
        ensure!(
            sorted_signatories.len() >= threshold as usize,
            "Threshold must not be greater than the number of unique signatories"
        );
        ensure!(
            threshold >= 2,
            "Threshold must be at least 2 - for threshold 1, use `as_multi_threshold_1`"
        );

        Ok(Self {
            signatories: sorted_signatories,
            threshold,
        })
    }

    pub fn account(&self) -> AccountId {
        let entropy =
            (b"modlpy/utilisuba", &self.signatories, &self.threshold).using_encoded(blake2_256);
        Decode::decode(&mut TrailingZeroInput::new(entropy.as_ref()))
            .expect("infinite length input; no invalid inputs for type; qed")
    }
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct Context {
    party: MultisigParty,
    author: AccountId,

    timepoint: Timepoint,
    max_weight: Weight,

    call: Option<Call>,
    call_hash: CallHash,

    approvers: HashSet<AccountId>,
}

impl Context {
    pub fn change_max_weight(&mut self, max_weight: Weight) {
        self.max_weight = max_weight;
    }

    fn set_call(&mut self, call: &Call) -> anyhow::Result<()> {
        ensure!(
            self.call_hash == compute_call_hash(call),
            "Call doesn't match to the registered hash"
        );
        self.call = Some(call.clone());
        Ok(())
    }

    fn add_approval(mut self, approver: AccountId) -> Option<Self> {
        self.approvers.insert(approver);
        if self.approvers.len() >= (self.party.threshold as usize) {
            None
        } else {
            Some(self)
        }
    }
}

#[async_trait::async_trait]
pub trait MultisigApiExt {
    async fn get_timepoint(
        &self,
        party_account: &AccountId,
        call_hash: &CallHash,
        block_hash: Option<BlockHash>,
    ) -> Timepoint;
}

#[async_trait::async_trait]
impl<C: ConnectionApi> MultisigApiExt for C {
    async fn get_timepoint(
        &self,
        party_account: &AccountId,
        call_hash: &CallHash,
        block_hash: Option<BlockHash>,
    ) -> Timepoint {
        let multisigs = api::storage()
            .multisig()
            .multisigs(party_account, call_hash);
        let Multisig { when, .. } = self.get_storage_entry(&multisigs, block_hash).await;
        when
    }
}

#[async_trait::async_trait]
pub trait MultisigContextualApi {
    async fn initiate(
        &self,
        party: &MultisigParty,
        max_weight: &Weight,
        call_hash: CallHash,
        status: TxStatus,
    ) -> anyhow::Result<(BlockHash, Context)>;
    async fn initiate_with_call(
        &self,
        party: &MultisigParty,
        max_weight: &Weight,
        call: Call,
        status: TxStatus,
    ) -> anyhow::Result<(BlockHash, Context)>;
    async fn approve(
        &self,
        context: Context,
        status: TxStatus,
    ) -> anyhow::Result<(BlockHash, Option<Context>)>;
    async fn approve_with_call(
        &self,
        context: Context,
        call: Option<Call>,
        status: TxStatus,
    ) -> anyhow::Result<(BlockHash, Option<Context>)>;
    async fn cancel(&self, context: Context, status: TxStatus) -> anyhow::Result<BlockHash>;
}

#[async_trait::async_trait]
impl<S: SignedConnectionApi> MultisigContextualApi for S {
    async fn initiate(
        &self,
        party: &MultisigParty,
        max_weight: &Weight,
        call_hash: CallHash,
        status: TxStatus,
    ) -> AnyResult<(BlockHash, Context)> {
        let other_signatories = ensure_signer_in_party(self, party)?;

        let block_hash = self
            .approve_as_multi(
                party.threshold,
                other_signatories,
                None,
                max_weight.clone(),
                call_hash,
                status,
            )
            .await?;

        let timepoint = self
            .get_timepoint(&party.account(), &call_hash, Some(block_hash))
            .await;
        let author = account_from_keypair(self.signer().signer());

        Ok((
            block_hash,
            Context {
                party: party.clone(),
                author: author.clone(),
                timepoint,
                max_weight: max_weight.clone(),
                call: None,
                call_hash,
                approvers: HashSet::from([author]),
            },
        ))
    }

    async fn initiate_with_call(
        &self,
        party: &MultisigParty,
        max_weight: &Weight,
        call: Call,
        status: TxStatus,
    ) -> AnyResult<(BlockHash, Context)> {
        let other_signatories = ensure_signer_in_party(self, party)?;

        let block_hash = self
            .as_multi(
                party.threshold,
                other_signatories,
                None,
                max_weight.clone(),
                call.clone(),
                status,
            )
            .await?;

        let call_hash = compute_call_hash(&call);
        let timepoint = self
            .get_timepoint(&party.account(), &call_hash, Some(block_hash))
            .await;
        let author = account_from_keypair(self.signer().signer());

        Ok((
            block_hash,
            Context {
                party: party.clone(),
                author: author.clone(),
                timepoint,
                max_weight: max_weight.clone(),
                call: Some(call.clone()),
                call_hash,
                approvers: HashSet::from([author]),
            },
        ))
    }

    async fn approve(
        &self,
        context: Context,
        status: TxStatus,
    ) -> AnyResult<(BlockHash, Option<Context>)> {
        let other_signatories = ensure_signer_in_party(self, &context.party)?;

        self.approve_as_multi(
            context.party.threshold,
            other_signatories,
            Some(context.timepoint.clone()),
            context.max_weight.clone(),
            context.call_hash,
            status,
        )
        .await
        .map(|block_hash| {
            (
                block_hash,
                context.add_approval(account_from_keypair(self.signer().signer())),
            )
        })
    }

    async fn approve_with_call(
        &self,
        mut context: Context,
        call: Option<Call>,
        status: TxStatus,
    ) -> AnyResult<(BlockHash, Option<Context>)> {
        let other_signatories = ensure_signer_in_party(self, &context.party)?;

        let call = match (call.as_ref(), context.call.as_ref()) {
            (None, None) => Err(anyhow!(
                "Call wasn't provided earlier - you must pass it now"
            )),
            (None, Some(call)) => Ok(call),
            (Some(call), None) => {
                context.set_call(call)?;
                Ok(call)
            }
            (Some(saved_call), Some(new_call)) => {
                ensure!(
                    saved_call == new_call,
                    "The call is different that the one used previously"
                );
                Ok(new_call)
            }
        }?;

        self.as_multi(
            context.party.threshold,
            other_signatories,
            Some(context.timepoint.clone()),
            context.max_weight.clone(),
            call.clone(),
            status,
        )
        .await
        .map(|block_hash| {
            (
                block_hash,
                context.add_approval(account_from_keypair(self.signer().signer())),
            )
        })
    }

    async fn cancel(&self, context: Context, status: TxStatus) -> AnyResult<BlockHash> {
        let other_signatories = ensure_signer_in_party(self, &context.party)?;

        let signer = account_from_keypair(self.signer().signer());
        ensure!(
            signer == context.author,
            "Only the author can cancel multisig aggregation"
        );

        self.cancel_as_multi(
            context.party.threshold,
            other_signatories,
            context.timepoint,
            context.call_hash,
            status,
        )
        .await
    }
}

pub fn compute_call_hash(call: &Call) -> CallHash {
    call.using_encoded(blake2_256)
}

fn ensure_signer_in_party<S: SignedConnectionApi>(
    conn: &S,
    party: &MultisigParty,
) -> anyhow::Result<Vec<AccountId>> {
    let signer_account = account_from_keypair(conn.signer().signer());
    if let Ok(index) = party.signatories.binary_search(&signer_account) {
        let mut other_signatories = party.signatories.clone();
        other_signatories.remove(index);
        Ok(other_signatories)
    } else {
        Err(anyhow!("Connection should be signed by a party member"))
    }
}
