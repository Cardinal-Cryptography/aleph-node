use core::marker::PhantomData;
use std::{
    fmt::{Debug, Display, Error as FmtError, Formatter},
    iter,
};

use log::debug;
use primitives::BlockNumber;

use crate::{
    session::{SessionBoundaryInfo, SessionId},
    sync::{
        data::{BranchKnowledge, NetworkData, Request, State},
        forest::{Error as ForestError, Forest, InitializationError as ForestInitializationError},
        Block, BlockIdFor, BlockImport, ChainStatus, ChainStatusExt, ChainStatusExtError,
        FinalizationStatus, Finalizer, Header, IsAncestor, Justification, PeerId, Verifier,
        LOG_TARGET,
    },
    BlockIdentifier,
};

/// Handles for interacting with the blockchain database.
pub struct DatabaseIO<B, J, CS, F, BI>
where
    B: Block,
    J: Justification<Header = B::Header>,
    CS: ChainStatus<B, J>,
    F: Finalizer<J>,
    BI: BlockImport<B>,
{
    chain_status: CS,
    finalizer: F,
    block_importer: BI,
    _phantom: PhantomData<(B, J)>,
}

impl<B, J, CS, F, BI> DatabaseIO<B, J, CS, F, BI>
where
    B: Block,
    J: Justification<Header = B::Header>,
    CS: ChainStatus<B, J>,
    F: Finalizer<J>,
    BI: BlockImport<B>,
{
    pub fn new(chain_status: CS, finalizer: F, block_importer: BI) -> Self {
        Self {
            chain_status,
            finalizer,
            block_importer,
            _phantom: PhantomData,
        }
    }
}

fn into_vecs<B, J>(chunks: Vec<Chunk<B, J>>) -> (Vec<B>, Vec<J::Unverified>, Vec<J::Header>)
where
    J: Justification,
    B: Block<Header = J::Header>,
{
    let mut blocks = vec![];
    let mut headers = vec![];
    let mut justifications = vec![];

    for chunk in chunks {
        match chunk {
            Chunk::Blocks(bs) => blocks.extend(bs),
            Chunk::Justification(j) => justifications.push(j),
            Chunk::Headers(h) => headers.extend(h),
        }
    }

    (blocks, justifications, headers)
}

enum Chunk<B, J>
where
    J: Justification,
    B: Block<Header = J::Header>,
{
    Blocks(Vec<B>),
    Justification(J::Unverified),
    Headers(Vec<J::Header>),
}

struct NewState<J: Justification> {
    top_justification: BlockIdFor<J>,
    top_imported: BlockIdFor<J>,
}

/// Types used by the Handler. For improved readability.
pub trait HandlerTypes {
    /// What can go wrong when handling a piece of data.
    type Error;
}

/// Handler for data incoming from the network.
pub struct Handler<B, I, J, CS, V, F, BI>
where
    B: Block,
    I: PeerId,
    J: Justification<Header = B::Header>,
    CS: ChainStatus<B, J>,
    V: Verifier<J>,
    F: Finalizer<J>,
    BI: BlockImport<B>,
{
    chain_status: CS,
    verifier: V,
    finalizer: F,
    forest: Forest<I, J>,
    session_info: SessionBoundaryInfo,
    block_importer: BI,
    phantom: PhantomData<B>,
}

/// What actions can the handler recommend as a reaction to some data.
#[derive(Clone, Debug)]
pub enum HandleStateAction<B, J>
where
    B: Block,
    J: Justification,
{
    /// A response for the peer that sent us the data.
    Response(NetworkData<B, J>),
    /// A request for the highest justified block that should be performed periodically.
    HighestJustified(BlockIdFor<J>),
    /// Do nothing.
    Noop,
}

impl<B, J> HandleStateAction<B, J>
where
    B: Block,
    J: Justification,
{
    fn response(justification: J::Unverified, other_justification: Option<J::Unverified>) -> Self {
        Self::Response(NetworkData::StateBroadcastResponse(
            justification,
            other_justification,
        ))
    }
}

impl<B, J> From<Option<BlockIdFor<J>>> for HandleStateAction<B, J>
where
    B: Block,
    J: Justification,
{
    fn from(value: Option<BlockIdFor<J>>) -> Self {
        match value {
            Some(id) => Self::HighestJustified(id),
            None => Self::Noop,
        }
    }
}

/// What can go wrong when handling a piece of data.
#[derive(Clone, Debug)]
pub enum Error<B, J, CS, V, F>
where
    J: Justification,
    B: Block<Header = J::Header>,
    CS: ChainStatus<B, J>,
    V: Verifier<J>,
    F: Finalizer<J>,
{
    Verifier(V::Error),
    ChainStatus(CS::Error),
    ChainStatusExt(ChainStatusExtError<CS::Error, J>),
    Finalizer(F::Error),
    Forest(ForestError),
    ForestInitialization(ForestInitializationError<B, J, CS>),
    MissingJustification,
    BlockNotImportable,
}

impl<B, J, CS, V, F> Display for Error<B, J, CS, V, F>
where
    J: Justification,
    B: Block<Header = J::Header>,
    CS: ChainStatus<B, J>,
    V: Verifier<J>,
    F: Finalizer<J>,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), FmtError> {
        use Error::*;
        match self {
            Verifier(e) => write!(f, "verifier error: {}", e),
            ChainStatus(e) => write!(f, "chain status error: {}", e),
            Finalizer(e) => write!(f, "finalized error: {}", e),
            Forest(e) => write!(f, "forest error: {}", e),
            ForestInitialization(e) => write!(f, "forest initialization error: {}", e),
            MissingJustification => write!(
                f,
                "justification for the last block of a past session missing"
            ),
            BlockNotImportable => {
                write!(f, "cannot import a block that we do not consider required")
            }
            ChainStatusExt(e) => write!(f, "chain status error: {}", e),
        }
    }
}

impl<B, J, CS, V, F> From<ForestError> for Error<B, J, CS, V, F>
where
    J: Justification,
    B: Block<Header = J::Header>,
    CS: ChainStatus<B, J>,
    V: Verifier<J>,
    F: Finalizer<J>,
{
    fn from(e: ForestError) -> Self {
        Error::Forest(e)
    }
}

impl<B, J, CS, V, F> From<ChainStatusExtError<CS::Error, J>> for Error<B, J, CS, V, F>
where
    J: Justification,
    B: Block<Header = J::Header>,
    CS: ChainStatus<B, J>,
    V: Verifier<J>,
    F: Finalizer<J>,
{
    fn from(e: ChainStatusExtError<CS::Error, J>) -> Self {
        Error::ChainStatusExt(e)
    }
}

impl<B, I, J, CS, V, F, BI> HandlerTypes for Handler<B, I, J, CS, V, F, BI>
where
    B: Block,
    I: PeerId,
    J: Justification<Header = B::Header>,
    CS: ChainStatus<B, J>,
    V: Verifier<J>,
    F: Finalizer<J>,
    BI: BlockImport<B>,
{
    type Error = Error<B, J, CS, V, F>;
}

impl<B, I, J, CS, V, F, BI> Handler<B, I, J, CS, V, F, BI>
where
    B: Block,
    I: PeerId,
    J: Justification<Header = B::Header>,
    CS: ChainStatus<B, J>,
    V: Verifier<J>,
    F: Finalizer<J>,
    BI: BlockImport<B>,
{
    /// New handler with the provided chain interfaces.
    pub fn new(
        database_io: DatabaseIO<B, J, CS, F, BI>,
        verifier: V,
        session_info: SessionBoundaryInfo,
    ) -> Result<Self, <Self as HandlerTypes>::Error> {
        let DatabaseIO {
            chain_status,
            finalizer,
            block_importer,
            ..
        } = database_io;
        let forest = Forest::new(&chain_status).map_err(Error::ForestInitialization)?;
        Ok(Handler {
            chain_status,
            verifier,
            finalizer,
            forest,
            session_info,
            block_importer,
            phantom: PhantomData,
        })
    }

    fn try_finalize(&mut self) -> Result<(), <Self as HandlerTypes>::Error> {
        let mut number = self
            .chain_status
            .top_finalized()
            .map_err(Error::ChainStatus)?
            .header()
            .id()
            .number()
            + 1;
        loop {
            while let Some(justification) = self.forest.try_finalize(&number) {
                self.finalizer
                    .finalize(justification)
                    .map_err(Error::Finalizer)?;
                number += 1;
            }
            number = self
                .session_info
                .last_block_of_session(self.session_info.session_id_from_block_num(number));
            match self.forest.try_finalize(&number) {
                Some(justification) => {
                    self.finalizer
                        .finalize(justification)
                        .map_err(Error::Finalizer)?;
                    number += 1;
                }
                None => return Ok(()),
            };
        }
    }

    fn get_unverified_justification(
        &self,
        number: BlockNumber,
    ) -> Result<Option<J::Unverified>, <Self as HandlerTypes>::Error> {
        use FinalizationStatus::FinalizedWithJustification;

        match self
            .chain_status
            .finalized_at(number)
            .map_err(Error::ChainStatus)?
        {
            FinalizedWithJustification(j) => Ok(Some(j.into_unverified())),
            _ => Ok(None),
        }
    }

    /// Inform the handler that a block has been imported.
    pub fn block_imported(
        &mut self,
        header: J::Header,
    ) -> Result<(), <Self as HandlerTypes>::Error> {
        self.forest.update_body(&header)?;
        self.try_finalize()
    }

    fn next_justification(
        &self,
        last_justification_number: BlockNumber,
    ) -> Result<Option<J::Unverified>, <Self as HandlerTypes>::Error> {
        let next_number = last_justification_number + 1;

        match self.get_unverified_justification(next_number)? {
            Some(justification) => return Ok(Some(justification)),
            _ => {}
        }

        // either we have justification under `next_number`
        // or we have to check last block of the session for it.
        let last_block_of_session_number = self
            .session_info
            .last_block_of_session(self.session_info.session_id_from_block_num(next_number));

        match self.get_unverified_justification(last_block_of_session_number)? {
            Some(justification) => Ok(Some(justification)),
            _ => Ok(None),
        }
    }

    fn finalized_blocks_between(
        &self,
        mut from: BlockNumber,
        to: BlockNumber,
    ) -> Result<Vec<B>, <Self as HandlerTypes>::Error> {
        use Error::ChainStatus;

        let mut blocks = vec![];

        while from != to && from < to {
            let id = match self.chain_status.finalized_at(from + 1) {
                Ok(FinalizationStatus::NotFinalized) | Err(_) => break,
                Ok(FinalizationStatus::FinalizedWithJustification(j)) => j.header().id(),
                Ok(FinalizationStatus::FinalizedByDescendant(header)) => header.id(),
            };

            let block = match self.chain_status.block(id).map_err(ChainStatus)? {
                None => break,
                Some(b) => b,
            };
            from += 1;
            blocks.push(block);
        }

        Ok(blocks)
    }

    fn base_response(
        &mut self,
        their_top_justification: &BlockIdFor<J>,
        their_top_imported: &BlockIdFor<J>,
    ) -> Result<(Vec<Chunk<B, J>>, NewState<J>), <Self as HandlerTypes>::Error> {
        let mut chunks = vec![];
        let mut last_justification_sent = their_top_justification.clone();
        let mut last_block_sent = their_top_imported.clone();

        // helper, push chunk of blocks to chunks if there is non empty path of finalized blocks
        // between from and to. `From` is not included while `to` is included in the path - if from
        // is equal to `to` this result in empty path which is not appended to chunks.
        let blocks_chunk =
            |from, to, chunks: &mut Vec<_>| -> Result<(), <Self as HandlerTypes>::Error> {
                // append blocks up to justification in increasing order if they dont have them
                let blocks = self.finalized_blocks_between(from, to)?;
                if !blocks.is_empty() {
                    chunks.push(Chunk::Blocks(blocks));
                }
                Ok(())
            };

        // helper, push chunk of headers to chunks if there is non empty path of them
        // between from and to. Headers are added in reversed order
        let headers_chunk =
            |from, to, chunks: &mut Vec<_>| -> Result<(), <Self as HandlerTypes>::Error> {
                // append headers in reverse order, without justification.
                let path = self.chain_status.headers_path(&from, &to)?;
                if !path.is_empty() {
                    chunks.push(Chunk::Headers(path))
                }
                Ok(())
            };

        while let Some(justification) = self.next_justification(last_justification_sent.number())? {
            let justification_number = justification.id().number();

            blocks_chunk(last_block_sent.number(), justification_number, &mut chunks)?;
            headers_chunk(
                justification.id().clone(),
                last_justification_sent.clone(),
                &mut chunks,
            )?;

            if justification_number > last_block_sent.number() {
                last_block_sent = justification.id();
            }
            last_justification_sent = justification.id();

            chunks.push(Chunk::Justification(justification));
        }

        Ok((
            chunks,
            NewState {
                top_justification: last_justification_sent.clone(),
                top_imported: last_block_sent,
            },
        ))
    }

    // returns true if we are sure this block belongs to fork
    fn is_on_fork(&self, id: &BlockIdFor<J>) -> Result<bool, <Self as HandlerTypes>::Error> {
        use Error::*;
        let our_top_justified = self
            .chain_status
            .top_finalized()
            .map_err(ChainStatus)?
            .header()
            .id();

        let block = match self.chain_status.block(id.clone()).map_err(ChainStatus)? {
            Some(block) => block,
            // we dont have block but its below our top justified = fork
            None if id.number() <= our_top_justified.number() => return Ok(true),
            // we dont have block and its over our top justified = we cant say if it is a fork or not
            _ => return Ok(false),
        };

        // if the block is under the top finalized check if the finalized block at block.number is
        // equal to block
        if block.header().id().number() < our_top_justified.number() {
            match self
                .chain_status
                .finalized_at(block.header().id().number())
                .map_err(ChainStatus)?
            {
                FinalizationStatus::FinalizedWithJustification(j) => {
                    Ok(j.header().id() != block.header().id())
                }
                FinalizationStatus::FinalizedByDescendant(header) => {
                    Ok(header.id() != block.header().id())
                }
                FinalizationStatus::NotFinalized => Ok(true),
            }
        } else {
            // otherwise check if our top finalized block is ancestor of the block
            match self
                .chain_status
                .is_ancestor_of(&our_top_justified, &block.header().id())
            {
                // if ancestor is unknown then we are not sure if the block is on the fork
                IsAncestor::Yes | IsAncestor::Unknown => Ok(false),
                IsAncestor::No => Ok(true),
            }
        }
    }

    fn chunks_to_target(
        &self,
        target: BlockIdFor<J>,
        last_justification_sent: BlockIdFor<J>,
        last_block_sent: BlockIdFor<J>,
        last_header_known: BlockIdFor<J>,
    ) -> Result<Vec<Chunk<B, J>>, <Self as HandlerTypes>::Error> {
        let mut chunks = vec![];

        if target.number() < last_justification_sent.number() {
            return Ok(chunks);
        }

        let header_path = self
            .chain_status
            .headers_path(&last_header_known, &last_justification_sent)?;
        if !header_path.is_empty() {
            chunks.push(Chunk::Headers(header_path));
        }

        // 1. we send headers from their last known header to last_justification_sent
        // 2. we feed them blocks from bottom if possible
        if last_justification_sent == last_block_sent {
            let mut blocks = self.chain_status.block_path(&target, &last_block_sent)?;
            blocks.reverse();
            chunks.push(Chunk::Blocks(blocks));
        }

        Ok(chunks)
    }

    fn most_helpful_response(
        &mut self,
        their_top_justification: BlockIdFor<J>,
        their_top_imported: BlockIdFor<J>,
        their_last_known_header: BlockIdFor<J>,
        target: BlockIdFor<J>,
    ) -> Result<Vec<Chunk<B, J>>, <Self as HandlerTypes>::Error> {
        // smth is wrong with the request, just send the base response
        if target.number() < their_top_imported.number()
            || target.number() < their_last_known_header.number()
        {
            return self
                .base_response(&their_top_justification, &their_top_justification)
                .map(|(chunks, _)| chunks);
        }

        // their top imported is fork, just send the base response
        if self.is_on_fork(&their_top_imported)? {
            return self
                .base_response(&their_top_justification, &their_top_justification)
                .map(|(chunks, _)| chunks);
        }

        // their target is fork, send the base response but don't repeat blocks that they have
        if self.is_on_fork(&target)? {
            return self
                .base_response(&their_top_justification, &their_top_imported)
                .map(|(chunks, _)| chunks);
        }

        // their last known_header is fork, this means their target is also fork. send the base response
        if self.is_on_fork(&their_last_known_header)? {
            return self
                .base_response(&their_top_justification, &their_top_justification)
                .map(|(chunks, _)| chunks);
        }

        // Now we know:
        // * their top_imported is all gucci
        // * target is okey
        // * their last_known_header is also gucci
        // OR we dont know path that connects our state to theirs blocks.
        // In such case we can send base helpful response and extend it with additional information how to reach the target if we know it.

        let (mut base_chunks, their_new_state) =
            self.base_response(&their_top_justification, &their_top_imported)?;

        let rest = match self.chunks_to_target(
            target,
            their_new_state.top_justification,
            their_new_state.top_imported,
            their_last_known_header,
        ) {
            Ok(chunks) => chunks,
            Err(e) => {
                debug!(
                    target: LOG_TARGET,
                    "Could not compute rest of the chunks, {}.", e
                );
                vec![]
            }
        };

        base_chunks.extend(rest);

        Ok(base_chunks)
    }

    /// Handle a request for potentially substantial amounts of data.
    ///
    /// Oh deer.
    pub fn handle_request(
        &mut self,
        request: Request<J>,
    ) -> Result<Option<NetworkData<B, J>>, <Self as HandlerTypes>::Error> {
        let their_top_justified = request.state().top_justification().id();
        let target_id = request.target_id();
        let branch_knowledge = request.branch_knowledge();

        let (top_imported, last_known_header) = match branch_knowledge {
            BranchKnowledge::LowestId(id) => (their_top_justified.clone(), id.clone()),
            BranchKnowledge::TopImported(id) => (id.clone(), their_top_justified.clone()),
        };

        let chunks = self.most_helpful_response(
            their_top_justified,
            top_imported,
            last_known_header,
            target_id.clone(),
        )?;

        let maybe_response = match chunks.is_empty() {
            true => None,
            false => {
                let (blocks, justifications, headers) = into_vecs(chunks);
                Some(NetworkData::RequestResponse(
                    justifications,
                    headers,
                    blocks,
                ))
            }
        };

        Ok(maybe_response)
    }

    /// Handle a single unverified justification.
    /// Return `Some(id)` if this justification was higher than the previously known highest justification.
    fn handle_justification(
        &mut self,
        justification: J::Unverified,
        maybe_peer: Option<I>,
    ) -> Result<Option<BlockIdFor<J>>, <Self as HandlerTypes>::Error> {
        let justification = self
            .verifier
            .verify(justification)
            .map_err(Error::Verifier)?;
        let id = justification.header().id();
        let maybe_id = match self
            .forest
            .update_justification(justification, maybe_peer)?
        {
            true => Some(id),
            false => None,
        };
        self.try_finalize()?;
        Ok(maybe_id)
    }

    fn handle_justifications(
        &mut self,
        justifications: Vec<J::Unverified>,
        maybe_peer: Option<I>,
    ) -> (Option<BlockIdFor<J>>, Option<<Self as HandlerTypes>::Error>) {
        let mut maybe_id = None;
        for justification in justifications {
            maybe_id = match self.handle_justification(justification, maybe_peer.clone()) {
                Ok(maybe_other_id) => match (&maybe_id, &maybe_other_id) {
                    (None, _) => maybe_other_id,
                    (Some(id), Some(other_id)) if other_id.number() > id.number() => maybe_other_id,
                    _ => maybe_id,
                },
                Err(e) => return (maybe_id, Some(e)),
            };
        }
        (maybe_id, None)
    }

    /// Handle a justification from user returning the action we should take.
    pub fn handle_justification_from_user(
        &mut self,
        justification: J::Unverified,
    ) -> Result<Option<BlockIdFor<J>>, <Self as HandlerTypes>::Error> {
        self.handle_justification(justification, None)
    }

    /// Handle a state response returning the action we should take, and possibly an error.
    pub fn handle_state_response(
        &mut self,
        justification: J::Unverified,
        maybe_justification: Option<J::Unverified>,
        peer: I,
    ) -> (Option<BlockIdFor<J>>, Option<<Self as HandlerTypes>::Error>) {
        self.handle_justifications(
            iter::once(justification)
                .chain(maybe_justification)
                .collect(),
            Some(peer),
        )
    }

    /// Handle a request response returning the action we should take, and possibly an error.
    pub fn handle_request_response(
        &mut self,
        justifications: Vec<J::Unverified>,
        headers: Vec<J::Header>,
        blocks: Vec<B>,
        peer: I,
    ) -> (Option<BlockIdFor<J>>, Option<<Self as HandlerTypes>::Error>) {
        // handle justifications
        let sync_action = match self.handle_justifications(justifications, Some(peer.clone())) {
            (sync_action, None) => sync_action,
            (sync_action, Some(e)) => return (sync_action, Some(e)),
        };

        // handle headers
        for header in headers {
            if let Err(e) = self
                .forest
                .update_required_header(&header, Some(peer.clone()))
            {
                return (sync_action, Some(Error::Forest(e)));
            }
        }

        // handle blocks
        for block in blocks {
            match self.forest.importable(&block.header().id()) {
                true => self.block_importer.import_block(block),
                false => return (sync_action, Some(Error::BlockNotImportable)),
            }
        }

        (sync_action, None)
    }

    fn last_justification_unverified(
        &self,
        session: SessionId,
    ) -> Result<J::Unverified, <Self as HandlerTypes>::Error> {
        use Error::*;
        Ok(self
            .chain_status
            .finalized_at(self.session_info.last_block_of_session(session))
            .map_err(ChainStatus)?
            .has_justification()
            .ok_or(MissingJustification)?
            .into_unverified())
    }

    /// Handle a state broadcast returning the actions we should take in response.
    pub fn handle_state(
        &mut self,
        state: State<J>,
        peer: I,
    ) -> Result<HandleStateAction<B, J>, <Self as HandlerTypes>::Error> {
        use Error::*;
        let remote_top_number = state.top_justification().id().number();
        let local_top = self.chain_status.top_finalized().map_err(ChainStatus)?;
        let local_top_number = local_top.header().id().number();
        let remote_session = self
            .session_info
            .session_id_from_block_num(remote_top_number);
        let local_session = self
            .session_info
            .session_id_from_block_num(local_top_number);
        match local_session.0.checked_sub(remote_session.0) {
            // remote session number larger than ours, we can try to import the justification
            None => Ok(self
                .handle_justification(state.top_justification(), Some(peer))?
                .into()),
            // same session
            Some(0) => match remote_top_number >= local_top_number {
                // remote top justification higher than ours, we can import the justification
                true => Ok(self
                    .handle_justification(state.top_justification(), Some(peer))?
                    .into()),
                // remote top justification lower than ours, we can send a response
                false => Ok(HandleStateAction::response(
                    local_top.into_unverified(),
                    None,
                )),
            },
            // remote lags one session behind
            Some(1) => Ok(HandleStateAction::response(
                self.last_justification_unverified(remote_session)?,
                Some(local_top.into_unverified()),
            )),
            // remote lags multiple sessions behind
            Some(2..) => Ok(HandleStateAction::response(
                self.last_justification_unverified(remote_session)?,
                Some(self.last_justification_unverified(SessionId(remote_session.0 + 1))?),
            )),
        }
    }

    /// The current state of our database.
    pub fn state(&self) -> Result<State<J>, <Self as HandlerTypes>::Error> {
        let top_justification = self
            .chain_status
            .top_finalized()
            .map_err(Error::ChainStatus)?
            .into_unverified();
        Ok(State::new(top_justification))
    }

    /// The forest held by this handler, read only.
    pub fn forest(&self) -> &Forest<I, J> {
        &self.forest
    }

    /// Handle an internal block request.
    /// Returns `true` if this was the first time something indicated interest in this block.
    pub fn handle_internal_request(
        &mut self,
        id: &BlockIdFor<J>,
    ) -> Result<bool, <Self as HandlerTypes>::Error> {
        let should_request = self.forest.update_block_identifier(id, None, true)?;

        Ok(should_request)
    }
}

#[cfg(test)]
mod tests {
    use super::{DatabaseIO, HandleStateAction, Handler};
    use crate::{
        session::SessionBoundaryInfo,
        sync::{
            data::{BranchKnowledge::*, NetworkData, Request},
            mock::{Backend, MockBlock, MockHeader, MockJustification, MockPeerId, MockVerifier},
            ChainStatus, Header, Justification,
        },
        BlockIdentifier, SessionPeriod,
    };

    type MockHandler =
        Handler<MockBlock, MockPeerId, MockJustification, Backend, MockVerifier, Backend, Backend>;

    const SESSION_PERIOD: usize = 20;

    fn setup() -> (MockHandler, Backend, impl Send) {
        let (backend, _keep) = Backend::setup(SESSION_PERIOD);
        let verifier = MockVerifier {};
        let database_io = DatabaseIO::new(backend.clone(), backend.clone(), backend.clone());
        let handler = Handler::new(
            database_io,
            verifier,
            SessionBoundaryInfo::new(SessionPeriod(20)),
        )
        .expect("mock backend works");
        (handler, backend, _keep)
    }

    fn import_branch(backend: &Backend, branch_length: usize) -> Vec<MockHeader> {
        let result: Vec<_> = backend
            .best_block()
            .expect("mock backend works")
            .random_branch()
            .take(branch_length)
            .collect();
        for header in &result {
            backend.import(header.clone());
        }
        result
    }

    #[test]
    fn finalizes_imported_and_justified() {
        let (mut handler, backend, _keep) = setup();
        let header = import_branch(&backend, 1)[0].clone();
        handler
            .block_imported(header.clone())
            .expect("importing in order");
        let justification = MockJustification::for_header(header);
        let peer = rand::random();
        assert!(
            handler
                .handle_justification(justification.clone().into_unverified(), Some(peer))
                .expect("correct justification")
                == Some(justification.id())
        );
        assert_eq!(
            backend.top_finalized().expect("mock backend works"),
            justification
        );
    }

    #[test]
    fn requests_missing_justifications_without_blocks() {
        let (mut handler, backend, _keep) = setup();
        let peer = rand::random();
        // skip the first justification, now every next added justification
        // should spawn a new task
        for justification in import_branch(&backend, 5)
            .into_iter()
            .map(MockJustification::for_header)
            .skip(1)
        {
            assert!(
                handler
                    .handle_justification(justification.clone().into_unverified(), Some(peer))
                    .expect("correct justification")
                    == Some(justification.id())
            );
        }
    }

    #[test]
    fn requests_missing_justifications_with_blocks() {
        let (mut handler, backend, _keep) = setup();
        let peer = rand::random();
        let justifications: Vec<MockJustification> = import_branch(&backend, 5)
            .into_iter()
            .map(MockJustification::for_header)
            .collect();
        for justification in justifications.iter() {
            handler
                .block_imported(justification.header().clone())
                .expect("importing in order");
        }
        // skip the first justification, now every next added justification
        // should spawn a new task
        for justification in justifications.into_iter().skip(1) {
            assert!(
                handler
                    .handle_justification(justification.clone().into_unverified(), Some(peer))
                    .expect("correct justification")
                    == Some(justification.id())
            );
        }
    }

    #[test]
    fn initializes_forest_properly() {
        let (backend, _keep) = Backend::setup(SESSION_PERIOD);
        let header = import_branch(&backend, 1)[0].clone();
        // header already imported, Handler should initialize Forest properly
        let verifier = MockVerifier {};
        let database_io = DatabaseIO::new(backend.clone(), backend.clone(), backend.clone());
        let mut handler = Handler::new(
            database_io,
            verifier,
            SessionBoundaryInfo::new(SessionPeriod(20)),
        )
        .expect("mock backend works");
        let justification = MockJustification::for_header(header);
        let peer: MockPeerId = rand::random();
        assert!(
            handler
                .handle_justification(justification.clone().into_unverified(), Some(peer))
                .expect("correct justification")
                == Some(justification.id())
        );
        // should be auto-finalized, if Forest knows about imported body
        assert_eq!(
            backend.top_finalized().expect("mock backend works"),
            justification
        );
    }

    #[test]
    fn finalizes_justified_and_imported() {
        let (mut handler, backend, _keep) = setup();
        let header = import_branch(&backend, 1)[0].clone();
        let justification = MockJustification::for_header(header.clone());
        let peer = rand::random();
        match handler
            .handle_justification(justification.clone().into_unverified(), Some(peer))
            .expect("correct justification")
        {
            Some(id) => assert_eq!(id, header.id()),
            None => panic!("expected an id, got nothing"),
        }
        handler.block_imported(header).expect("importing in order");
        assert_eq!(
            backend.top_finalized().expect("mock backend works"),
            justification
        );
    }

    #[test]
    fn handles_state_with_large_difference() {
        let (mut handler, backend, _keep) = setup();
        let initial_state = handler.state().expect("state works");
        let peer = rand::random();
        let justifications: Vec<MockJustification> = import_branch(&backend, 43)
            .into_iter()
            .map(MockJustification::for_header)
            .collect();
        let last_from_first_session = justifications[18].clone().into_unverified();
        let last_from_second_session = justifications[38].clone().into_unverified();
        for justification in justifications.into_iter() {
            handler
                .block_imported(justification.header().clone())
                .expect("importing in order");
            handler
                .handle_justification(justification.clone().into_unverified(), Some(peer))
                .expect("correct justification");
        }
        match handler
            .handle_state(initial_state, peer)
            .expect("correct justification")
        {
            HandleStateAction::Response(NetworkData::StateBroadcastResponse(
                justification,
                maybe_justification,
            )) => {
                assert_eq!(justification, last_from_first_session);
                assert_eq!(maybe_justification, Some(last_from_second_session));
            }
            other_action => panic!(
                "expected a response with justifications, got {:?}",
                other_action
            ),
        }
    }

    #[test]
    fn handles_state_with_medium_difference() {
        let (mut handler, backend, _keep) = setup();
        let initial_state = handler.state().expect("state works");
        let peer = rand::random();
        let justifications: Vec<MockJustification> = import_branch(&backend, 23)
            .into_iter()
            .map(MockJustification::for_header)
            .collect();
        let last_from_first_session = justifications[18].clone().into_unverified();
        let top = justifications[22].clone().into_unverified();
        for justification in justifications.into_iter() {
            handler
                .block_imported(justification.header().clone())
                .expect("importing in order");
            handler
                .handle_justification(justification.clone().into_unverified(), Some(peer))
                .expect("correct justification");
        }
        match handler
            .handle_state(initial_state, peer)
            .expect("correct justification")
        {
            HandleStateAction::Response(NetworkData::StateBroadcastResponse(
                justification,
                maybe_justification,
            )) => {
                assert_eq!(justification, last_from_first_session);
                assert_eq!(maybe_justification, Some(top));
            }
            other_action => panic!(
                "expected a response with justifications, got {:?}",
                other_action
            ),
        }
    }

    #[test]
    fn handles_state_with_small_difference() {
        let (mut handler, backend, _keep) = setup();
        let initial_state = handler.state().expect("state works");
        let peer = rand::random();
        let justifications: Vec<MockJustification> = import_branch(&backend, 13)
            .into_iter()
            .map(MockJustification::for_header)
            .collect();
        let top = justifications[12].clone().into_unverified();
        for justification in justifications.into_iter() {
            handler
                .block_imported(justification.header().clone())
                .expect("importing in order");
            handler
                .handle_justification(justification.clone().into_unverified(), Some(peer))
                .expect("correct justification");
        }
        match handler
            .handle_state(initial_state, peer)
            .expect("correct justification")
        {
            HandleStateAction::Response(NetworkData::StateBroadcastResponse(
                justification,
                maybe_justification,
            )) => {
                assert_eq!(justification, top);
                assert!(maybe_justification.is_none());
            }
            other_action => panic!(
                "expected a response with justifications, got {:?}",
                other_action
            ),
        }
    }

    #[test]
    fn handles_request() {
        let (mut handler, backend, _keep) = setup();
        let initial_state = handler.state().expect("state works");
        let peer = rand::random();
        let mut justifications: Vec<_> = import_branch(&backend, 500)
            .into_iter()
            .map(MockJustification::for_header)
            .collect();
        for justification in &justifications {
            let number = justification.header().id().number();
            handler
                .block_imported(justification.header().clone())
                .expect("importing in order");
            // skip some justifications, but always keep the last of the session
            // justifications right before the last will be skipped in response
            if number % 20 < 10 || number % 20 > 14 {
                handler
                    .handle_justification(justification.clone().into_unverified(), Some(peer))
                    .expect("correct justification");
            }
        }
        // currently ignored, so picking a random one
        let requested_id = justifications[43].header().id();
        let request = Request::new(requested_id.clone(), LowestId(requested_id), initial_state);
        // filter justifications, these are supposed to be included in the response
        justifications.retain(|j| {
            let number = j.header().id().number();
            number % 20 < 10 || number % 20 == 19
        });
        match handler.handle_request(request).expect("correct request") {
            Some(NetworkData::RequestResponse(sent_justifications, _, _)) => {
                assert_eq!(sent_justifications.len(), 100);
                for (sent_justification, justification) in
                    sent_justifications.iter().zip(justifications)
                {
                    assert_eq!(
                        sent_justification.header().id(),
                        justification.header().id()
                    );
                }
            }
            other_action => panic!(
                "expected a response with justifications, got {:?}",
                other_action
            ),
        }
    }

    #[test]
    fn handles_new_internal_request() {
        let (mut handler, backend, _keep) = setup();
        let _ = handler.state().expect("state works");
        let headers = import_branch(&backend, 2);

        assert!(handler.handle_internal_request(&headers[1].id()).unwrap());
        assert!(!handler.handle_internal_request(&headers[1].id()).unwrap());
    }
}
