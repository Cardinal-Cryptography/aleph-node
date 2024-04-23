//! Implementations and definitions of traits used in legacy abft
use crate::{
    block::{Header, HeaderVerifier, UnverifiedHeader},
    data_io::{AlephData, ChainInfoProvider, DataProvider, OrderedDataInterpreter},
};

#[async_trait::async_trait]
impl<UH: UnverifiedHeader> legacy_aleph_bft::DataProvider<AlephData<UH>> for DataProvider<UH> {
    async fn get_data(&mut self) -> Option<AlephData<UH>> {
        DataProvider::get_data(self).await
    }
}

impl<CIP, H, V> legacy_aleph_bft::FinalizationHandler<AlephData<H::Unverified>>
    for OrderedDataInterpreter<CIP, H, V>
where
    CIP: ChainInfoProvider,
    H: Header,
    V: HeaderVerifier<H>,
{
    fn data_finalized(
        &mut self,
        data: AlephData<H::Unverified>,
        _creator: legacy_aleph_bft::NodeIndex,
    ) {
        OrderedDataInterpreter::data_finalized(self, data)
    }
}
