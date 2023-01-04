use primitives::{Balance, BlockNumber};

use crate::{
    aleph_runtime::RuntimeCall, api, api::runtime_types, sp_weights::weight_v2::Weight, AccountId,
    BlockHash, SignedConnectionApi, TxStatus,
};

pub type CallHash = [u8; 32];
pub type Call = RuntimeCall;
pub type Timepoint = runtime_types::pallet_multisig::Timepoint<BlockNumber>;
pub type Multisig = runtime_types::pallet_multisig::Multisig<BlockNumber, Balance, AccountId>;

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
        threshold: u16,
        other_signatories: Vec<AccountId>,
        timepoint: Option<Timepoint>,
        max_weight: Weight,
        call: Call,
        status: TxStatus,
    ) -> anyhow::Result<BlockHash>;
    async fn approve_as_multi(
        &self,
        threshold: u16,
        other_signatories: Vec<AccountId>,
        timepoint: Option<Timepoint>,
        max_weight: Weight,
        call_hash: CallHash,
        status: TxStatus,
    ) -> anyhow::Result<BlockHash>;
    async fn cancel_as_multi(
        &self,
        threshold: u16,
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
        threshold: u16,
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
        threshold: u16,
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
        threshold: u16,
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
