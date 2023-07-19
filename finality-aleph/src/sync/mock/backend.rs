use std::{
    collections::{HashMap, VecDeque},
    fmt::{Display, Error as FmtError, Formatter},
    sync::Arc,
};

use futures::channel::mpsc::{self, UnboundedSender};
use parking_lot::Mutex;

use crate::{
    sync::{
        mock::{MockBlock, MockHeader, MockIdentifier, MockJustification, MockNotification},
        Block, BlockImport, BlockStatus, ChainStatus, ChainStatusNotifier, FinalizationStatus,
        Finalizer, Header, Justification as JustificationT,
    },
    BlockIdentifier,
};

#[derive(Clone, Debug)]
struct BackendStorage {
    session_period: u32,
    blockchain: HashMap<MockIdentifier, MockBlock>,
    finalized: Vec<MockIdentifier>,
    best_block: MockIdentifier,
}

#[derive(Clone, Debug)]
pub struct Backend {
    inner: Arc<Mutex<BackendStorage>>,
    notification_sender: UnboundedSender<MockNotification>,
}

fn is_predecessor(
    storage: &HashMap<MockIdentifier, MockBlock>,
    mut header: MockHeader,
    maybe_predecessor: MockIdentifier,
) -> bool {
    while let Some(parent) = header.parent_id() {
        if header.id().number() != parent.number() + 1 {
            break;
        }
        if parent == maybe_predecessor {
            return true;
        }

        header = match storage.get(&parent) {
            Some(block) => block.header().clone(),
            None => return false,
        }
    }
    false
}

impl Backend {
    pub fn setup(session_period: usize) -> (Self, impl ChainStatusNotifier<MockHeader>) {
        let (notification_sender, notification_receiver) = mpsc::unbounded();

        (
            Backend::new(notification_sender, session_period as u32),
            notification_receiver,
        )
    }

    fn new(notification_sender: UnboundedSender<MockNotification>, session_period: u32) -> Self {
        let header = MockHeader::random_parentless(0);
        let id = header.id();

        let block = MockBlock {
            header: header.clone(),
            justification: Some(MockJustification::for_header(header)),
        };

        let storage = Arc::new(Mutex::new(BackendStorage {
            session_period,
            blockchain: HashMap::from([(id.clone(), block)]),
            finalized: vec![id.clone()],
            best_block: id,
        }));

        Self {
            inner: storage,
            notification_sender,
        }
    }

    fn notify_imported(&self, header: MockHeader) {
        self.notification_sender
            .unbounded_send(MockNotification::BlockImported(header))
            .expect("notification receiver is open");
    }

    fn notify_finalized(&self, header: MockHeader) {
        self.notification_sender
            .unbounded_send(MockNotification::BlockFinalized(header))
            .expect("notification receiver is open");
    }

    pub fn import(&self, header: MockHeader) {
        let mut storage = self.inner.lock();

        let parent_id = match header.parent_id() {
            Some(id) => id,
            None => panic!("importing block without a parent: {header:?}"),
        };

        if storage.blockchain.contains_key(&header.id()) {
            panic!("importing an already imported block: {header:?}")
        }

        if !storage.blockchain.contains_key(&parent_id) {
            panic!("importing block without an imported parent: {header:?}")
        }

        if header.id().number() != parent_id.number() + 1 {
            panic!("importing block without a correct parent: {header:?}")
        }

        if header.id().number() > storage.best_block.number()
            && is_predecessor(
                &storage.blockchain,
                header.clone(),
                storage.best_block.clone(),
            )
        {
            storage.best_block = header.id();
        }

        storage
            .blockchain
            .insert(header.id(), MockBlock::new(header.clone()));

        self.notify_imported(header);
    }
}

#[derive(Debug)]
pub struct FinalizerError;

impl Display for FinalizerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), FmtError> {
        write!(f, "{self:?}")
    }
}

impl Finalizer<MockJustification> for Backend {
    type Error = FinalizerError;

    fn finalize(&self, justification: MockJustification) -> Result<(), Self::Error> {
        if !justification.is_correct {
            panic!("finalizing block with an incorrect justification: {justification:?}");
        }

        let mut storage = self.inner.lock();

        let header = justification.header();
        let parent_id = match justification.header().parent_id() {
            Some(id) => id,
            None => panic!("finalizing block without specified parent: {header:?}"),
        };

        if storage.blockchain.get(&parent_id).is_none() {
            panic!("finalizing block without imported parent: {header:?}")
        }

        let header = justification.header().clone();
        let finalizing_id = header.id();
        let block = match storage.blockchain.get_mut(&finalizing_id) {
            Some(block) => block,
            None => panic!("finalizing a not imported block: {header:?}"),
        };

        block.finalize(justification);

        let last_number = match storage.finalized.last() {
            Some(id) => id.number,
            None => 0,
        };

        // Check if the previous block was finalized, or this is the last block of the current
        // session
        let allowed_numbers = match storage.finalized.last() {
            Some(id) => [
                id.number + 1,
                id.number + storage.session_period
                    - (id.number + 1).rem_euclid(storage.session_period),
            ],
            None => [0, storage.session_period - 1],
        };

        if !allowed_numbers.contains(&finalizing_id.number) {
            panic!("finalizing a block that is not a child of top finalized (round {:?}), nor the last of a session (round {:?}): round {:?}", allowed_numbers[0], allowed_numbers[1], finalizing_id.number);
        }

        let mut blocks_to_finalize = VecDeque::new();
        let mut block_to_finalize = finalizing_id.clone();

        // Finalize also blocks that lead up to last finalized
        while block_to_finalize.number != last_number {
            blocks_to_finalize.push_front(block_to_finalize.clone());
            block_to_finalize = storage
                .blockchain
                .get(&block_to_finalize)
                .expect("We already checked that")
                .header
                .parent
                .clone()
                .expect("We already checked parent exists");
        }

        // Actually check if we are not finalizing fork
        let first_to_finalize = blocks_to_finalize
            .front()
            .expect("At least one block is being finalized");

        let parent_of_first_block_to_finalize = storage
            .blockchain
            .get(first_to_finalize)
            .expect("We already checked that")
            .header
            .parent
            .clone()
            .expect("We already checked parent exists");
        let last_finalized = storage
            .finalized
            .last()
            .expect("At least one block is always finalized");
        if parent_of_first_block_to_finalize != *last_finalized {
            panic!("finalizing a block that is not a child of top finalized.");
        }

        storage.finalized.extend(blocks_to_finalize);
        // In case finalization changes best block, we set best block, to top finalized.
        // Whenever a new import happens, best block will update anyway.
        if !is_predecessor(
            &storage.blockchain,
            storage
                .blockchain
                .get(&storage.best_block)
                .unwrap()
                .header()
                .clone(),
            finalizing_id.clone(),
        ) {
            storage.best_block = finalizing_id
        }
        self.notify_finalized(header);

        Ok(())
    }
}

impl BlockImport<MockBlock> for Backend {
    fn import_block(&mut self, block: MockBlock) {
        self.import(block.header);
    }
}

#[derive(Debug)]
pub struct StatusError;

impl Display for StatusError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), FmtError> {
        write!(f, "{self:?}")
    }
}

impl ChainStatus<MockBlock, MockJustification> for Backend {
    type Error = StatusError;

    fn status_of(&self, id: MockIdentifier) -> Result<BlockStatus<MockJustification>, Self::Error> {
        let storage = self.inner.lock();
        let block = match storage.blockchain.get(&id) {
            Some(block) => block,
            None => return Ok(BlockStatus::Unknown),
        };

        if let Some(justification) = block.justification.clone() {
            Ok(BlockStatus::Justified(justification))
        } else {
            Ok(BlockStatus::Present(block.header().clone()))
        }
    }

    fn block(&self, id: MockIdentifier) -> Result<Option<MockBlock>, Self::Error> {
        Ok(self.inner.lock().blockchain.get(&id).cloned())
    }

    fn finalized_at(
        &self,
        number: u32,
    ) -> Result<FinalizationStatus<MockJustification>, Self::Error> {
        use FinalizationStatus::*;

        if number > self.top_finalized()?.header.id.number {
            return Ok(NotFinalized);
        }

        let storage = self.inner.lock();
        let id = match storage.finalized.get(number as usize) {
            Some(id) => id,
            None => return Ok(NotFinalized),
        };

        let block = storage.blockchain.get(id).ok_or(StatusError)?;

        if let Some(j) = block.justification.clone() {
            return Ok(FinalizedWithJustification(j));
        }

        Ok(FinalizedByDescendant(block.header.clone()))
    }

    fn best_block(&self) -> Result<MockHeader, Self::Error> {
        let storage = self.inner.lock();
        let id = storage.best_block.clone();
        storage
            .blockchain
            .get(&id)
            .map(|b| b.header().clone())
            .ok_or(StatusError)
    }

    fn top_finalized(&self) -> Result<MockJustification, Self::Error> {
        let storage = self.inner.lock();
        let id = storage
            .finalized
            .last()
            .expect("there is a top finalized")
            .clone();
        storage
            .blockchain
            .get(&id)
            .and_then(|b| b.justification.clone())
            .ok_or(StatusError)
    }

    fn children(&self, id: MockIdentifier) -> Result<Vec<MockHeader>, Self::Error> {
        match self.status_of(id.clone())? {
            BlockStatus::Unknown => Err(StatusError),
            _ => {
                let storage = self.inner.lock();
                for (stored_id, block) in storage.blockchain.iter() {
                    if stored_id.number() == id.number + 1 {
                        return Ok(Vec::from([block.header().clone()]));
                    }
                }
                Ok(Vec::new())
            }
        }
    }
}
