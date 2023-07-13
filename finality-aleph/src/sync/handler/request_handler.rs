use core::{default::Default, marker::PhantomData};
use std::{
    collections::VecDeque,
    fmt::{Display, Formatter},
};

use primitives::BlockNumber;

use crate::{
    session::{SessionBoundaryInfo, SessionId},
    sync::{
        data::BranchKnowledge, handler::Request, Block, BlockIdFor, BlockStatus, ChainStatus,
        FinalizationStatus, Header, Justification,
    },
    BlockIdentifier,
};

#[derive(Debug, Clone)]
pub enum RequestHandlerError<T: Display> {
    RootMismatch,
    MissingParent,
    BadState,
    BadRequest,
    ChainStatusBadState,
    ChainStatusError(T),
}

impl<T: Display> From<T> for RequestHandlerError<T> {
    fn from(value: T) -> Self {
        RequestHandlerError::ChainStatusError(value)
    }
}

impl<T: Display> Display for RequestHandlerError<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            RequestHandlerError::RootMismatch => write!(f, "root mismatch"),
            RequestHandlerError::MissingParent => write!(f, "missing parent"),
            RequestHandlerError::ChainStatusError(e) => write!(f, "{}", e),
            RequestHandlerError::BadState => write!(f, "bad state"),
            RequestHandlerError::BadRequest => write!(f, "bad request"),
            RequestHandlerError::ChainStatusBadState => write!(f, "bad state of chain_status"),
        }
    }
}

type HandlerResult<T, Error> = Result<T, RequestHandlerError<Error>>;

pub enum Chunk<B, J>
where
    B: Block,
    J: Justification<Header = B::Header>,
{
    Justification(J::Unverified),
    Blocks(Vec<B>),
    Headers(Vec<J::Header>),
}

pub fn into_vecs<B, J>(chunks: Vec<Chunk<B, J>>) -> (Vec<B>, Vec<J::Unverified>, Vec<J::Header>)
where
    J: Justification,
    B: Block<Header = J::Header>,
{
    let mut blocks = vec![];
    let mut headers = vec![];
    let mut justifications = vec![];

    for chunk in chunks {
        match chunk {
            Chunk::Blocks(mut bs) => {
                bs.extend(blocks);
                blocks = bs;
            }
            Chunk::Justification(j) => justifications.push(j),
            Chunk::Headers(h) => headers.extend(h),
        }
    }

    (blocks, justifications, headers)
}

#[derive(Debug)]
enum HeadOfChunk<B, J>
where
    B: Block,
    J: Justification<Header = B::Header>,
{
    Justification(J, Option<B>, Option<J::Header>),
    Header(J::Header),
}

#[derive(Default)]
struct PreChunk<B, J>
where
    B: Block,
    J: Justification<Header = B::Header>,
{
    pub just: Option<J>,
    pub blocks: VecDeque<B>,
    pub headers: Vec<J::Header>,
}

impl<B, J> PreChunk<B, J>
where
    B: Block,
    J: Justification<Header = B::Header>,
{
    fn new() -> Self {
        Self {
            just: None,
            blocks: VecDeque::new(),
            headers: vec![],
        }
    }

    fn to_chunks(self) -> Vec<Chunk<B, J>> {
        let mut chunks = vec![];

        if let Some(j) = self.just {
            chunks.push(Chunk::Justification(j.into_unverified()));
        }

        if !self.headers.is_empty() {
            chunks.push(Chunk::Headers(self.headers));
        }

        if !self.blocks.is_empty() {
            chunks.push(Chunk::Blocks(self.blocks.into()));
        }

        chunks
    }

    fn add_block(&mut self, b: Option<B>) {
        if let Some(b) = b {
            self.blocks.push_front(b);
        }
    }

    fn add_header(&mut self, h: Option<J::Header>) {
        if let Some(h) = h {
            self.headers.push(h);
        }
    }

    fn add_justification(&mut self, j: Option<J>) {
        self.just = j;
    }
}

#[derive(PartialEq, Eq, Debug)]
enum State {
    EverythingButHeader,
    Everything,
    OnlyJustification,
    Finished,
}

pub struct RequestHandler<'a, B, J, CS>
where
    B: Block,
    J: Justification<Header = B::Header>,
    CS: ChainStatus<B, J>,
{
    chain_status: &'a CS,
    session_info: &'a SessionBoundaryInfo,
    _phantom: PhantomData<(B, J)>,
}

impl<'a, B, J, CS> RequestHandler<'a, B, J, CS>
where
    B: Block,
    J: Justification<Header = B::Header>,
    CS: ChainStatus<B, J>,
{
    pub fn new(chain_status: &'a CS, session_info: &'a SessionBoundaryInfo) -> Self {
        Self {
            chain_status,
            session_info,
            _phantom: PhantomData,
        }
    }

    fn upper_limit(&self, id: BlockIdFor<J>) -> BlockNumber {
        let session = self.session_info.session_id_from_block_num(id.number());
        self.session_info
            .last_block_of_session(SessionId(session.0 + 1))
    }

    fn get_block(&self, id: BlockIdFor<J>) -> HandlerResult<Option<B>, CS::Error> {
        let b = self.chain_status.block(id)?;

        Ok(b)
    }

    fn get_justification_block_and_header(
        &self,
        id: BlockIdFor<J>,
    ) -> HandlerResult<(Option<J>, Option<B>, Option<J::Header>), CS::Error> {
        let (justification, block) = self.get_justification_and_block(id.clone())?;
        let header = block.as_ref().map(|b| b.header().clone());
        Ok((justification, block, header))
    }

    fn get_justification_and_block(
        &self,
        id: BlockIdFor<J>,
    ) -> HandlerResult<(Option<J>, Option<B>), CS::Error> {
        let justification = self.get_justification(id.clone())?;
        let block = self.chain_status.block(id.clone())?;
        Ok((justification, block))
    }

    fn get_justification(&self, id: BlockIdFor<J>) -> HandlerResult<Option<J>, CS::Error> {
        match self.chain_status.status_of(id.clone())? {
            BlockStatus::Justified(justification) => Ok(Some(justification)),
            BlockStatus::Present(_) => Ok(None),
            BlockStatus::Unknown => Err(RequestHandlerError::RootMismatch),
        }
    }

    fn parent_header(&self, header: &J::Header) -> HandlerResult<J::Header, CS::Error> {
        let parent_header = header
            .parent_id()
            .ok_or(RequestHandlerError::MissingParent)?;
        self.chain_status
            .header(parent_header)?
            .ok_or(RequestHandlerError::MissingParent)
    }

    fn step(
        &self,
        state: State,
        from: HeadOfChunk<B, J>,
        to: &BlockIdFor<J>,
        branch_knowledge: &BranchKnowledge<J>,
    ) -> HandlerResult<(State, HeadOfChunk<B, J>, Vec<Chunk<B, J>>), CS::Error> {
        let mut pre_chunk = PreChunk::new();
        let mut head_chunk = from;
        let mut state = state;

        loop {
            let header = match &head_chunk {
                HeadOfChunk::Justification(j, b, header) => {
                    pre_chunk.add_block(b.clone());
                    pre_chunk.add_header(header.clone());
                    pre_chunk.add_justification(Some(j.clone()));
                    self.parent_header(j.header())?
                }
                HeadOfChunk::Header(header) => header.clone(),
            };

            state = match branch_knowledge {
                BranchKnowledge::TopImported(id) if *id == header.id() => State::OnlyJustification,
                _ => state,
            };

            if *to == header.id() {
                state = State::Finished;
                break;
            }

            if to.number() >= header.id().number() {
                return Err(RequestHandlerError::RootMismatch);
            }

            head_chunk = match state {
                State::EverythingButHeader => {
                    let (j, b) = self.get_justification_and_block(header.id())?;
                    match j {
                        Some(j) => HeadOfChunk::Justification(j, b, None),
                        None => {
                            pre_chunk.add_block(b);
                            HeadOfChunk::Header(self.parent_header(&header)?)
                        }
                    }
                }
                State::Everything => {
                    let (j, b, h) = self.get_justification_block_and_header(header.id())?;
                    match j {
                        Some(j) => HeadOfChunk::Justification(j, b, h),
                        None => {
                            pre_chunk.add_block(b);
                            pre_chunk.add_header(h);
                            HeadOfChunk::Header(self.parent_header(&header)?)
                        }
                    }
                }
                State::OnlyJustification => {
                    let j = self.get_justification(header.id())?;
                    match j {
                        Some(j) => HeadOfChunk::Justification(j, None, None),
                        None => HeadOfChunk::Header(self.parent_header(&header)?),
                    }
                }
                State::Finished => return Err(RequestHandlerError::BadState),
            };

            state = match branch_knowledge {
                BranchKnowledge::LowestId(id) if *id == header.id() => State::Everything,
                _ => state,
            };

            match &head_chunk {
                HeadOfChunk::Justification(_, _, _) => break,
                _ => {}
            }
        }

        Ok((state, head_chunk, pre_chunk.to_chunks()))
    }

    fn chunks(
        self,
        head: HeadOfChunk<B, J>,
        branch_knowledge: BranchKnowledge<J>,
        to: BlockIdFor<J>,
    ) -> HandlerResult<Vec<Chunk<B, J>>, CS::Error> {
        let mut chunks = vec![];

        let mut head = head;
        let mut state = State::EverythingButHeader;

        while state != State::Finished {
            let (new_state, new_head, chunks_from_step) =
                self.step(state, head, &to, &branch_knowledge)?;

            chunks.extend(chunks_from_step);
            state = new_state;
            head = new_head;
        }

        Ok(chunks)
    }

    pub fn response(
        self,
        request: Request<J>,
    ) -> Result<(Vec<Chunk<B, J>>, bool), RequestHandlerError<CS::Error>> {
        if !request.is_valid() {
            return Err(RequestHandlerError::BadRequest);
        }

        let our_top_justification = self.chain_status.top_finalized()?;
        let top_justification = request.state().top_justification();
        let target = request.target_id();

        let upper_limit = self.upper_limit(top_justification.id());

        // request too far into future
        if target.number() > upper_limit {
            return Ok((vec![], false));
        }

        let head = match self.chain_status.status_of(target.clone())? {
            BlockStatus::Unknown => return Ok((vec![], true)),
            BlockStatus::Justified(justification) => {
                let b = self.get_block(justification.header().id())?;
                HeadOfChunk::Justification(justification, b, None)
            }
            BlockStatus::Present(header) => HeadOfChunk::Header(header),
        };

        let our_top_justification_number = our_top_justification.header().id().number();

        // set the target to either target, our last justified block or finalized block
        // at upper_limit height.
        let head = if target.number() > our_top_justification_number {
            head
        } else if upper_limit > our_top_justification_number {
            let b = self.get_block(our_top_justification.header().id())?;
            HeadOfChunk::Justification(our_top_justification, b, None)
        } else {
            match self.chain_status.finalized_at(upper_limit)? {
                FinalizationStatus::FinalizedWithJustification(j) => {
                    let b = self.get_block(j.header().id())?;
                    HeadOfChunk::Justification(j, b, None)
                }
                FinalizationStatus::FinalizedByDescendant(header) => HeadOfChunk::Header(header),
                FinalizationStatus::NotFinalized => {
                    return Err(RequestHandlerError::ChainStatusBadState)
                }
            }
        };

        let chunks = self.chunks(
            head,
            request.branch_knowledge().clone(),
            top_justification.id(),
        )?;

        Ok((chunks, false))
    }
}
