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

/// A group of accounts together with a threshold.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct MultisigParty {
    signatories: Vec<AccountId>,
    threshold: MultisigThreshold,
}

impl MultisigParty {
    /// Create new party from `signatories` and `threshold`.
    ///
    /// `signatories` can contain duplicates and doesn't have to be sorted. However, there must be
    /// at least 2 unique accounts. There is also a virtual upper bound - `MaxSignatories` constant.
    /// It isn't checked here (since it requires client), however, using too big party will fail
    /// when performing any chain interaction.
    ///
    /// `threshold` must be between 2 and number of unique accounts in `signatories`. For threshold
    /// 1, use special method `MultisigUserApi::as_multi_threshold_1`.
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

    /// The multisig account derived from signatories and threshold.
    ///
    /// This method is copied from the pallet, because:
    ///  -  we don't want to add a new dependency
    ///  -  we cannot instantiate pallet object here anyway (the corresponding functionality exists
    ///     as pallet's method rather than standalone function)
    pub fn account(&self) -> AccountId {
        let entropy =
            (b"modlpy/utilisuba", &self.signatories, &self.threshold).using_encoded(blake2_256);
        Decode::decode(&mut TrailingZeroInput::new(entropy.as_ref()))
            .expect("infinite length input; no invalid inputs for type; qed")
    }
}

/// A context in which ongoing signature aggregation is performed.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct Context {
    /// The entity for which aggregation is being made.
    party: MultisigParty,
    /// Derived multisig account (the source of the target call).
    author: AccountId,

    /// Pallet's coordinate for this aggregation.
    timepoint: Timepoint,
    /// Weight limit when dispatching the call.
    max_weight: Weight,

    /// The target dispatchable, if already provided.
    call: Option<Call>,
    /// The hash of the target dispatchable.
    call_hash: CallHash,

    /// The set of accounts, that already approved the call (via this context object), including the
    /// author.
    ///
    /// `approvers.len() < party.threshold` always holds.
    approvers: HashSet<AccountId>,
}

impl Context {
    /// In case `Context` object has been passed somewhere, where this limit should be adjusted, we
    /// allow for that.
    ///
    /// Actually, this isn't used until threshold is met, so such changing is perfectly safe.
    pub fn change_max_weight(&mut self, max_weight: Weight) {
        self.max_weight = max_weight;
    }

    /// Set `call` only if `self.call_hash` is matching.
    fn set_call(&mut self, call: &Call) -> anyhow::Result<()> {
        ensure!(
            self.call_hash == compute_call_hash(call),
            "Call doesn't match to the registered hash"
        );
        self.call = Some(call.clone());
        Ok(())
    }

    /// Register another approval. Consume `self` if the threshold has been met and `call` is
    /// already known.
    fn add_approval(mut self, approver: AccountId) -> Option<Self> {
        self.approvers.insert(approver);
        if self.call.is_some() && self.approvers.len() >= (self.party.threshold as usize) {
            None
        } else {
            Some(self)
        }
    }
}

/// Pallet multisig functionality that is not directly related to any pallet call.
#[async_trait::async_trait]
pub trait MultisigApiExt {
    /// Get the coordinate that corresponds to the ongoing signature aggregation for `party_account`
    /// and `call_hash`.
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

/// Pallet multisig API, but suited for cases when the whole scenario is performed in a single place
/// - we keep data in a context object which helps in concise programming.
#[async_trait::async_trait]
pub trait MultisigContextualApi {
    /// Start signature aggregation for `party` and `call_hash`. Get `Context` object as a result
    /// (together with standard block hash).
    ///
    /// This is the recommended way of initialization.
    async fn initiate(
        &self,
        party: &MultisigParty,
        max_weight: &Weight,
        call_hash: CallHash,
        status: TxStatus,
    ) -> anyhow::Result<(BlockHash, Context)>;
    /// Start signature aggregation for `party` and `call`. Get `Context` object as a result
    /// (together with standard block hash).
    ///
    /// Note: it is usually a better idea to pass `call` only with the final approval (so that it
    /// isn't stored on-chain).
    async fn initiate_with_call(
        &self,
        party: &MultisigParty,
        max_weight: &Weight,
        call: Call,
        status: TxStatus,
    ) -> anyhow::Result<(BlockHash, Context)>;
    /// Express contextual approval for the call hash. Get `Context` object back if the target
    /// dispatchable couldn't have been executed yet (either too less approvals or only hash is
    /// known).
    ///
    /// This is the recommended way for every intermediate approval.
    async fn approve(
        &self,
        context: Context,
        status: TxStatus,
    ) -> anyhow::Result<(BlockHash, Option<Context>)>;
    /// Express contextual approval for the `call`. Get `Context` object back if the threshold is
    /// still not met.
    ///
    /// This is the recommended way only for the final approval.
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

        // Even though `subxt` allows us to get timepoint when waiting for the submission
        // confirmation (see e.g. `ExtrinsicEvents` object that is returned from
        // `wait_for_finalized_success`), we chose to perform one additional storage read.
        // Firstly, because of brevity here (we would have to duplicate some lines from
        // `connections` module. Secondly, if `Timepoint` struct change, this method (reading raw
        // extrinsic position) might become incorrect.
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

/// Compute hash of `call`.
pub fn compute_call_hash(call: &Call) -> CallHash {
    call.using_encoded(blake2_256)
}

/// Ensure that the signer of `conn` is present in `party.signatories`. If so, return all other
/// signatories.
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
