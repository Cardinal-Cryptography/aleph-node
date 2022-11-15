use sp_core::H256;

use crate::{api, Connection};

pub type FeeMultiplier = u128;

#[async_trait::async_trait]
pub trait TransactionPaymentApi {
    async fn get_next_fee_multiplier(&self, at: Option<H256>) -> FeeMultiplier;
}

#[async_trait::async_trait]
impl TransactionPaymentApi for Connection {
    async fn get_next_fee_multiplier(&self, at: Option<H256>) -> FeeMultiplier {
        let addrs = api::storage().transaction_payment().next_fee_multiplier();

        match self.get_storage_entry_maybe(&addrs, at).await {
            Some(fm) => fm.0,
            None => 1,
        }
    }
}
