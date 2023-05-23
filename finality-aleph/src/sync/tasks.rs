use std::{
    collections::HashSet,
    fmt::{Display, Error as FmtError, Formatter},
    time::Duration,
};

use rand::{thread_rng, Rng};

use crate::{
    sync::{
        data::BranchKnowledge,
        forest::{Forest, Interest},
        BlockIdFor, Header, Justification, PeerId,
    },
    BlockIdentifier,
};

const MIN_DELAY: Duration = Duration::from_millis(300);
const ADDITIONAL_DELAY: Duration = Duration::from_millis(200);

fn delay_for_attempt(attempt: u32) -> Duration {
    MIN_DELAY
        + ADDITIONAL_DELAY
            .mul_f32(thread_rng().gen())
            .saturating_mul(attempt)
}

/// A task for requesting blocks. Keeps track of how many times it was executed and what kind of
/// request it is, highest justified or not.
pub struct RequestTask<BI: BlockIdentifier> {
    id: BI,
    highest_justified: bool,
    tries: u32,
}

impl<BI: BlockIdentifier> Display for RequestTask<BI> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), FmtError> {
        match self.highest_justified {
            true => write!(
                f,
                "highest justified request for {:?}, attempt {}",
                self.id, self.tries
            ),
            false => write!(f, "block request for {:?}, attempt {}", self.id, self.tries),
        }
    }
}

type PreRequestFor<I, J> = (BlockIdFor<J>, BranchKnowledge<J>, HashSet<I>);
type DelayedTask<BI> = (RequestTask<BI>, Duration);

/// What do to with the task, either ignore or perform a request and add a delayed task.
pub enum ProcessingResult<I: PeerId, J: Justification> {
    Ignore,
    Request(PreRequestFor<I, J>, DelayedTask<BlockIdFor<J>>),
}

impl<BI: BlockIdentifier> RequestTask<BI> {
    fn new(id: BI, highest_justified: bool) -> Self {
        RequestTask {
            id,
            highest_justified,
            tries: 0,
        }
    }

    /// A new task for requesting highest justified block with the provided ID.
    pub fn new_highest_justified(id: BI) -> Self {
        RequestTask::new(id, true)
    }

    /// Process the task using the information from the forest.
    pub fn process<I, J>(self, forest: &Forest<I, J>) -> ProcessingResult<I, J>
    where
        I: PeerId,
        J: Justification,
        J::Header: Header<Identifier = BI>,
    {
        use Interest::*;
        let RequestTask {
            id,
            highest_justified,
            tries,
        } = self;
        match (forest.request_interest(&id), highest_justified) {
            (
                Required {
                    know_most,
                    branch_knowledge,
                },
                false,
            )
            | (
                HighestJustified {
                    know_most,
                    branch_knowledge,
                },
                true,
            ) => {
                // Every second time we request from a random peer rather than the one we expect to
                // have it.
                let know_most = match tries % 2 == 0 {
                    true => know_most,
                    false => HashSet::new(),
                };
                let tries = tries + 1;
                ProcessingResult::Request(
                    (id.clone(), branch_knowledge, know_most),
                    (
                        RequestTask {
                            id: id.clone(),
                            highest_justified,
                            tries,
                        },
                        delay_for_attempt(tries),
                    ),
                )
            }
            _ => ProcessingResult::Ignore,
        }
    }
}
