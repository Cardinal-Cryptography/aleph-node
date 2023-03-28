use sp_blockchain::Error;
use sp_runtime::{traits::Block, Justification};

use crate::{
    finalization::BlockFinalizer,
    testing::mocks::{single_action_mock::SingleActionMock, TBlock},
    BlockHashNum, HashNum,
};
type CallArgs = (BlockHashNum<TBlock>, Justification);

#[derive(Clone, Default)]
pub struct MockedBlockFinalizer {
    mock: SingleActionMock<CallArgs>,
}

impl MockedBlockFinalizer {
    pub fn new() -> Self {
        Self {
            mock: Default::default(),
        }
    }

    pub async fn has_not_been_invoked(&self) -> bool {
        self.mock.has_not_been_invoked().await
    }

    pub async fn has_been_invoked_with(&self, block: TBlock) -> bool {
        self.mock
            .has_been_invoked_with(|(HashNum { hash, num }, _)| {
                block.hash() == hash && block.header.number == num
            })
            .await
    }
}

impl BlockFinalizer<BlockHashNum<TBlock>> for MockedBlockFinalizer {
    fn finalize_block(
        &self,
        block: BlockHashNum<TBlock>,
        justification: Justification,
    ) -> Result<(), Error> {
        self.mock.invoke_with((block, justification));
        Ok(())
    }
}
