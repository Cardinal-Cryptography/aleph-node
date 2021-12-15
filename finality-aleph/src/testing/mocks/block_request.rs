use sp_runtime::traits::Block;

use crate::network::RequestBlocks;
use crate::testing::mocks::single_action_mock::SingleActionMock;
use crate::testing::mocks::{TBlock, THash, TNumber};

type CallArgs = (THash, TNumber);

#[derive(Clone)]
pub(crate) struct MockedBlockRequester {
    mock: SingleActionMock<CallArgs>,
}

impl MockedBlockRequester {
    pub(crate) fn new() -> Self {
        Self {
            mock: Default::default(),
        }
    }

    pub(crate) async fn has_not_been_invoked(&self) -> bool {
        self.mock.has_not_been_invoked().await
    }

    pub(crate) async fn has_been_invoked_with(&self, block: TBlock) -> bool {
        self.mock
            .has_been_invoked_with(|(hash, number)| {
                block.hash() == hash && block.header.number == number
            })
            .await
    }
}

impl RequestBlocks<TBlock> for MockedBlockRequester {
    fn request_justification(&self, hash: &THash, number: TNumber) {
        self.mock.invoke_with((*hash, number))
    }

    fn request_stale_block(&self, _hash: THash, _number: TNumber) {
        panic!("`request_stale_block` not implemented!")
    }
}
