use std::{collections::HashSet, marker::PhantomData};

use super::{Header, Justification, PeerID};

pub enum Error {
    ContentCorrupted,
    InvalidHeader,
    HeaderMissingParentID,
}

#[derive(Clone, std::cmp::PartialEq, std::cmp::Eq)]
pub enum Importance {
    Auxiliary,
    TopRequired,
    Required,
    Imported,
}

#[derive(Clone, std::cmp::PartialEq, std::cmp::Eq)]
pub enum Content {
    Empty,
    Header,
    Justification,
}

type BlockIdFor<J> = <<J as Justification>::Header as Header>::Identifier;

pub struct TransitionSummary {
    pub gained_parent: bool,
    pub gained_justification: bool,
}

impl TransitionSummary {
    fn just_created() -> Self {
        Self {
            gained_parent: false,
            gained_justification: false,
        }
    }
}

struct TransitionSummaryMaker<I, J> {
    content_before: Content,
    importance_before: Importance,
    phantom: PhantomData<(I, J)>,
}

impl<I: PeerID, J: Justification> TransitionSummaryMaker<I, J> {
    fn new(vertex_before: &Vertex<I, J>) -> Result<Self, Error> {
        Ok(Self {
            content_before: vertex_before.content()?,
            importance_before: vertex_before.importance.clone(),
            phantom: PhantomData,
        })
    }

    fn make(self, vertex_after: &Vertex<I, J>) -> Result<Option<TransitionSummary>, Error> {
        use Content::*;
        let (content_after, importance_after) = vertex_after.state()?;
        Ok(
            if self.content_before == content_after && self.importance_before == importance_after {
                None
            } else {
                let (gained_parent, gained_justification) =
                    match (self.content_before, content_after) {
                        (Empty, Header) => (true, false),
                        (Empty, Justification) => (true, true),
                        (Header, Justification) => (false, true),
                        _ => (false, false),
                    };
                Some(TransitionSummary {
                    gained_parent,
                    gained_justification,
                })
            },
        )
    }
}

pub struct Vertex<I: PeerID, J: Justification> {
    know_most: HashSet<I>,
    importance: Importance,
    parent: Option<BlockIdFor<J>>,
    justification: Option<J>,
}

impl<I: PeerID, J: Justification> Vertex<I, J> {
    pub fn new() -> (Self, TransitionSummary) {
        (
            Self {
                know_most: HashSet::new(),
                importance: Importance::Auxiliary,
                parent: None,
                justification: None,
            },
            TransitionSummary::just_created(),
        )
    }

    fn content(&self) -> Result<Content, Error> {
        match (&self.parent, &self.justification) {
            (Some(_), Some(_)) => Ok(Content::Justification),
            (Some(_), None) => Ok(Content::Header),
            (None, Some(_)) => Err(Error::ContentCorrupted),
            (None, None) => Ok(Content::Empty),
        }
    }

    pub fn state(&self) -> Result<(Content, Importance), Error> {
        Ok((self.content()?, self.importance.clone()))
    }

    pub fn is_full(&self) -> Result<bool, Error> {
        Ok(self.state()? == (Content::Justification, Importance::Imported))
    }

    pub fn is_imported(&self) -> bool {
        self.importance == Importance::Imported
    }

    pub fn know_most(&self) -> &HashSet<I> {
        &self.know_most
    }

    pub fn parent(&self) -> &Option<BlockIdFor<J>> {
        &self.parent
    }

    pub fn justification(self) -> Option<J> {
        self.justification
    }

    pub fn add_holder(&mut self, holder: Option<I>) -> bool {
        match (self.content(), holder) {
            (Ok(Content::Empty), Some(peer_id)) | (Ok(Content::Header), Some(peer_id)) => {
                self.know_most.insert(peer_id)
            }
            _ => false,
        }
    }

    pub fn add_justification_holder(&mut self, holder: Option<I>) -> bool {
        match holder {
            Some(peer_id) => self.know_most.insert(peer_id),
            None => false,
        }
    }

    // STATE CHANGING METHODS

    pub fn try_set_top_required(&mut self) -> Result<Option<TransitionSummary>, Error> {
        use Importance::*;
        let summary_maker = TransitionSummaryMaker::new(&*self)?;
        if self.importance == Auxiliary {
            self.importance = TopRequired;
        }
        summary_maker.make(&*self)
    }

    pub fn try_set_required(&mut self) -> Result<Option<TransitionSummary>, Error> {
        use Importance::*;
        let summary_maker = TransitionSummaryMaker::new(&*self)?;
        match self.importance {
            Auxiliary | TopRequired => self.importance = Required,
            _ => (),
        };
        summary_maker.make(&*self)
    }

    pub fn try_insert_header(
        &mut self,
        header: &J::Header,
    ) -> Result<Option<TransitionSummary>, Error> {
        let summary_maker = TransitionSummaryMaker::new(&*self)?;
        let parent_id = header.parent_id().ok_or(Error::HeaderMissingParentID)?;
        match &self.parent {
            Some(id) => {
                if id != &parent_id {
                    return Err(Error::InvalidHeader);
                };
            }
            None => self.parent = Some(parent_id),
        }
        summary_maker.make(&*self)
    }

    pub fn try_insert_body(
        &mut self,
        header: &J::Header,
    ) -> Result<Option<TransitionSummary>, Error> {
        let summary_maker = TransitionSummaryMaker::new(&*self)?;
        self.try_insert_header(header)?;
        self.importance = Importance::Imported;
        summary_maker.make(&*self)
    }

    pub fn try_insert_justification(
        &mut self,
        justification: J,
    ) -> Result<Option<TransitionSummary>, Error> {
        let summary_maker = TransitionSummaryMaker::new(&*self)?;
        self.try_insert_header(justification.header())?;
        if self.justification.is_none() {
            self.justification = Some(justification);
            self.know_most.clear();
        }
        summary_maker.make(&*self)
    }
}
