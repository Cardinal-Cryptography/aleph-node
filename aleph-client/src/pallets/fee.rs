use crate::{api, BlockHash, Connection};

/// An alias for a fee multiplier.
pub type FeeMultiplier = u128;

/// Any object that implements transaction payment pallet api.
#[async_trait::async_trait]
pub trait TransactionPaymentApi {
    /// API for [`next_fee_multiplier`](https://paritytech.github.io/substrate/master/pallet_transaction_payment/pallet/struct.Pallet.html#method.next_fee_multiplier) call.
    async fn get_next_fee_multiplier(&self, at: Option<BlockHash>) -> FeeMultiplier;
}

#[async_trait::async_trait]
impl TransactionPaymentApi for Connection {
    async fn get_next_fee_multiplier(&self, at: Option<BlockHash>) -> FeeMultiplier {
        let addrs = api::storage().transaction_payment().next_fee_multiplier();

        match self.get_storage_entry_maybe(&addrs, at).await {
            Some(fm) => fm.0,
            None => 1,
        }
    }
}
