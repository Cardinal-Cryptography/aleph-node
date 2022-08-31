use codec::Compact;
use primitives::Balance;
use sp_core::{Pair, H256};
use sp_runtime::MultiAddress;
use substrate_api_client::{
    compose_call, compose_extrinsic, error::Error as SacError,
    extrinsic::balances::BalanceTransferXt, AccountId, ExtrinsicParams, GenericAddress, XtStatus,
};

use crate::{
    send_xt, try_send_xt, AnyConnection, BalanceTransfer, BatchTransactions, Extrinsic,
    SignedConnection,
};

pub type TransferTransaction = Extrinsic<([u8; 2], MultiAddress<AccountId, ()>, Compact<u128>)>;

pub fn transfer(
    connection: &SignedConnection,
    target: &AccountId,
    value: u128,
    status: XtStatus,
) -> TransferTransaction {
    let xt = connection
        .as_connection()
        .balance_transfer(GenericAddress::Id(target.clone()), value);
    send_xt(connection, xt.clone(), Some("transfer"), status);
    xt
}

pub fn batch_transfer(
    connection: &SignedConnection,
    account_keys: Vec<AccountId>,
    endowment: u128,
) {
    let batch_endow = account_keys
        .into_iter()
        .map(|account_id| {
            compose_call!(
                connection.as_connection().metadata,
                "Balances",
                "transfer",
                GenericAddress::Id(account_id),
                Compact(endowment)
            )
        })
        .collect::<Vec<_>>();

    let xt = compose_extrinsic!(connection.as_connection(), "Utility", "batch", batch_endow);
    send_xt(
        connection,
        xt,
        Some("batch of endow balances"),
        XtStatus::InBlock,
    );
}

impl BalanceTransfer for SignedConnection {
    type TransferTx =
        BalanceTransferXt<<crate::ExtrinsicParams as ac_primitives::ExtrinsicParams>::SignedExtra>;
    type Error = SacError;

    fn create_transfer_tx(&self, account: AccountId, amount: Balance) -> Self::TransferTx {
        self.as_connection()
            .balance_transfer(GenericAddress::Id(account), amount)
    }

    fn transfer(
        &self,
        tx: Self::TransferTx,
        status: XtStatus,
    ) -> Result<Option<H256>, Self::Error> {
        try_send_xt(self, tx, Some("transfer"), status)
    }
}

impl BatchTransactions<<SignedConnection as BalanceTransfer>::TransferTx> for SignedConnection {
    type Error = SacError;

    fn batch_and_send_transactions(
        &self,
        transactions: impl IntoIterator<Item = <SignedConnection as BalanceTransfer>::TransferTx>,
        status: XtStatus,
    ) -> Result<Option<H256>, Self::Error> {
        let txs = Vec::from_iter(transactions);
        let xt = compose_extrinsic!(self.as_connection(), "Utility", "batch", txs);
        try_send_xt(self, xt, Some("batch/transfer"), status)
    }
}
