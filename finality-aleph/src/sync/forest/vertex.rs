use std::collections::HashSet;

use super::{Justification, PeerID, HashNumber, Header};

pub enum Error {
    ContentCorrupted,
    InvalidTransition,
    InvalidHeader,
    InvalidJustification,
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

pub struct Vertex {
    know_most: HashSet<PeerID>,
    importance: Importance,
    parent: Option<HashNumber>,
    justification: Option<Justification>,
}

impl Vertex {
    pub fn new(holder: Option<PeerID>) -> Self {
        let know_most = match holder {
            Some(peer_id) => HashSet::from([peer_id]),
            None => HashSet::new(),
        };
        Vertex {
            know_most,
            importance: Importance::Auxiliary,
            parent: None,
            justification: None,
        }
    }

    fn content(&self) -> Result<Content, Error> {
        match (self.parent, self.justification) {
            (Some(_), Some(_)) => Ok(Content::Justification),
            (Some(_), None) => Ok(Content::Header),
            (None, Some(_)) => Err(Error::ContentCorrupted),
            (None, None) => Ok(Content::Empty),
        }
    }

    pub fn state(&self) -> Result<(Content, Importance), Error> {
        Ok((self.content()?, self.importance.clone()))
    }

    pub fn know_most(&self) -> HashSet<PeerID> {
        self.know_most
    }

    pub fn parent(&self) -> Option<HashNumber> {
        self.parent
    }

    pub fn justification(self) -> Option<Justification> {
        self.justification
    }

    pub fn add_holder(&mut self, holder: Option<PeerID>) {
        if let Some(peer_id) = holder {
            self.know_most.insert(peer_id);
        };
    }

    // STATE CHANGING METHODS

    pub fn bump_required(&mut self, is_top: bool) -> Result<bool, Error> {
        use Importance::*;
        match (self.importance, is_top) {
            (Auxiliary, true) => {
                self.importance = Importance::TopRequired;
                Ok(true)
            },
            (Auxiliary, false) => {
                self.importance = Importance::Required;
                Ok(true)
            },
            (TopRequired, false) => {
                self.importance = Importance::Required;
                Ok(true)
            },
            (Required, true) => Err(Error::InvalidTransition),
            _ => Ok(false),
        }
    }

    pub fn insert_header(&mut self, header: Header) -> Result<bool, Error> {
        match self.parent {
            Some(hashnumber) => {
                if hashnumber != header.parent_hashnumber() {
                    return Err(Error::InvalidHeader);
                };
                Ok(false)
            },
            None => {
                self.parent = Some(header.parent_hashnumber());
                Ok(true)
            },
        }
    }

    pub fn insert_body(&mut self, header: Header) -> Result<bool, Error> {
        Ok(self.insert_header(header)? || match self.importance {
            Importance::Imported => false,
            _ => {
                self.importance = Importance::Imported;
                true
            },
        })
    }

    pub fn insert_justification(&mut self, justification: Justification) -> Result<bool, Error> {
        Ok(self.insert_header(justification.header())? || match self.justification {
            Some(current_justification) => {
                if justification != current_justification {
                    return Err(Error::InvalidJustification);
                };
                false
            },
            None => {
                self.justification = Some(justification);
                true
            },
        })
    }
}
