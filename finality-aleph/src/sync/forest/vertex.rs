use std::collections::HashSet;

use super::{Header, Justification, JustificationWithParent, PeerID};

type BlockIDFor<J> = <<J as Justification>::Header as Header>::Identifier;

pub enum Error {
    InvalidParentID,
    JustificationImportance,
}

#[derive(Clone, std::cmp::PartialEq, std::cmp::Eq)]
pub enum EmptyImportance {
    Auxiliary,
    TopRequired,
    Required,
}

#[derive(Clone, std::cmp::PartialEq, std::cmp::Eq)]
pub enum HeaderImportance {
    Auxiliary,
    TopRequired,
    Required,
    Imported,
}

impl From<&EmptyImportance> for HeaderImportance {
    fn from(importance: &EmptyImportance) -> Self {
        match importance {
            EmptyImportance::Auxiliary => Self::Auxiliary,
            EmptyImportance::TopRequired => Self::TopRequired,
            EmptyImportance::Required => Self::Required,
        }
    }
}

#[derive(Clone, std::cmp::PartialEq, std::cmp::Eq)]
pub enum JustificationImportance {
    TopRequired,
    Required,
    Imported,
}

impl From<&HeaderImportance> for Option<JustificationImportance> {
    fn from(importance: &HeaderImportance) -> Self {
        match importance {
            HeaderImportance::Auxiliary => None,
            HeaderImportance::TopRequired => Some(JustificationImportance::TopRequired),
            HeaderImportance::Required => Some(JustificationImportance::Required),
            HeaderImportance::Imported => Some(JustificationImportance::Imported),
        }
    }
}

#[derive(Clone, std::cmp::PartialEq, std::cmp::Eq)]
pub enum Importance {
    Auxiliary,
    TopRequired,
    Required,
    Imported,
}

impl From<&EmptyImportance> for Importance {
    fn from(importance: &EmptyImportance) -> Self {
        match importance {
            EmptyImportance::Auxiliary => Self::Auxiliary,
            EmptyImportance::TopRequired => Self::TopRequired,
            EmptyImportance::Required => Self::Required,
        }
    }
}

impl From<&HeaderImportance> for Importance {
    fn from(importance: &HeaderImportance) -> Self {
        match importance {
            HeaderImportance::Auxiliary => Self::Auxiliary,
            HeaderImportance::TopRequired => Self::TopRequired,
            HeaderImportance::Required => Self::Required,
            HeaderImportance::Imported => Self::Imported,
        }
    }
}

impl From<&JustificationImportance> for Importance {
    fn from(importance: &JustificationImportance) -> Self {
        match importance {
            JustificationImportance::TopRequired => Self::TopRequired,
            JustificationImportance::Required => Self::Required,
            JustificationImportance::Imported => Self::Imported,
        }
    }
}

pub struct EmptyVertex<I: PeerID> {
    importance: EmptyImportance,
    know_most: HashSet<I>,
}

pub struct HeaderVertex<I: PeerID, J: Justification> {
    importance: HeaderImportance,
    parent: BlockIDFor<J>,
    know_most: HashSet<I>,
}

impl<I: PeerID, J: Justification> HeaderVertex<I, J> {
    fn is_imported(&self) -> bool {
        self.importance == HeaderImportance::Imported
    }
}

pub struct JustificationVertex<I: PeerID, J: Justification> {
    importance: JustificationImportance,
    justification_with_parent: JustificationWithParent<J>,
    know_most: HashSet<I>,
}

impl<I: PeerID, J: Justification> JustificationVertex<I, J> {
    fn is_imported(&self) -> bool {
        self.importance == JustificationImportance::Imported
    }
}

#[derive(Clone, std::cmp::PartialEq, std::cmp::Eq)]
pub enum State {
    Empty(EmptyImportance),
    Header(HeaderImportance),
    Justification(JustificationImportance),
}

impl State {
    pub fn importance(&self) -> Importance {
        use State::*;
        match self {
            Empty(importance) => importance.into(),
            Header(importance) => importance.into(),
            Justification(importance) => importance.into(),
        }
    }
}

pub enum Vertex<I: PeerID, J: Justification> {
    Empty(EmptyVertex<I>),
    Header(HeaderVertex<I, J>),
    Justification(JustificationVertex<I, J>),
}

impl<I: PeerID, J: Justification> Vertex<I, J> {
    pub fn new(holder: Option<I>) -> Self {
        Self::Empty(EmptyVertex {
            importance: EmptyImportance::Auxiliary,
            know_most: HashSet::from_iter(holder.into_iter()),
        })
    }

    pub fn state(&self) -> State {
        use Vertex::*;
        match self {
            Empty(vertex) => State::Empty(vertex.importance.clone()),
            Header(vertex) => State::Header(vertex.importance.clone()),
            Justification(vertex) => State::Justification(vertex.importance.clone()),
        }
    }

    pub fn is_full(&self) -> bool {
        match self {
            Self::Justification(vertex) => vertex.is_imported(),
            _ => false,
        }
    }

    pub fn is_imported(&self) -> bool {
        match self {
            Self::Empty(_) => false,
            Self::Header(vertex) => vertex.is_imported(),
            Self::Justification(vertex) => vertex.is_imported(),
        }
    }

    pub fn parent(&self) -> Option<&BlockIDFor<J>> {
        match self {
            Self::Empty(_) => None,
            Self::Header(vertex) => Some(&vertex.parent),
            Self::Justification(vertex) => Some(&vertex.justification_with_parent.parent),
        }
    }

    pub fn justification(self) -> Option<J> {
        match self {
            Self::Justification(vertex) => Some(vertex.justification_with_parent.justification),
            _ => None,
        }
    }

    pub fn know_most(&self) -> &HashSet<I> {
        match self {
            Self::Empty(vertex) => &vertex.know_most,
            Self::Header(vertex) => &vertex.know_most,
            Self::Justification(vertex) => &vertex.know_most,
        }
    }

    pub fn try_set_top_required(&mut self) -> bool {
        match self {
            Self::Empty(vertex) => match vertex.importance {
                EmptyImportance::Auxiliary => {
                    vertex.importance = EmptyImportance::TopRequired;
                    true
                }
                _ => false,
            },
            Self::Header(vertex) => match vertex.importance {
                HeaderImportance::Auxiliary => {
                    vertex.importance = HeaderImportance::TopRequired;
                    true
                }
                _ => false,
            },
            Self::Justification(_) => false,
        }
    }

    pub fn try_set_required(&mut self) -> bool {
        match self {
            Self::Empty(vertex) => match vertex.importance {
                EmptyImportance::Auxiliary | EmptyImportance::TopRequired => {
                    vertex.importance = EmptyImportance::Required;
                    true
                }
                _ => false,
            },
            Self::Header(vertex) => match vertex.importance {
                HeaderImportance::Auxiliary | HeaderImportance::TopRequired => {
                    vertex.importance = HeaderImportance::Required;
                    true
                }
                _ => false,
            },
            Self::Justification(vertex) => match vertex.importance {
                JustificationImportance::TopRequired => {
                    vertex.importance = JustificationImportance::Required;
                    true
                }
                _ => false,
            },
        }
    }

    fn check_parent(&self, parent_id: &BlockIDFor<J>) -> Result<(), Error> {
        match self {
            Self::Empty(_) => Ok(()),
            Self::Header(vertex) => match parent_id == &vertex.parent {
                true => Ok(()),
                false => Err(Error::InvalidParentID),
            },
            Self::Justification(vertex) => {
                match parent_id == &vertex.justification_with_parent.parent {
                    true => Ok(()),
                    false => Err(Error::InvalidParentID),
                }
            }
        }
    }

    pub fn add_empty_holder(&mut self, holder: Option<I>) -> bool {
        if let Some(holder) = holder {
            if let Self::Empty(vertex) = self {
                return vertex.know_most.insert(holder);
            }
        }
        false
    }

    pub fn try_insert_header(
        &mut self,
        parent_id: BlockIDFor<J>,
        holder: Option<I>,
    ) -> Result<bool, Error> {
        self.check_parent(&parent_id)?;
        let output = match self {
            Self::Empty(vertex) => {
                *self = Self::Header(HeaderVertex {
                    importance: (&vertex.importance).into(),
                    parent: parent_id,
                    know_most: vertex.know_most.clone(),
                });
                Ok(true)
            }
            _ => Ok(false),
        };
        if let Some(holder) = holder {
            match self {
                Self::Empty(vertex) => vertex.know_most.insert(holder),
                Self::Header(vertex) => vertex.know_most.insert(holder),
                Self::Justification(_) => false,
            };
        }
        output
    }

    pub fn try_insert_body(
        &mut self,
        parent_id: BlockIDFor<J>,
        holder: Option<I>,
    ) -> Result<bool, Error> {
        self.check_parent(&parent_id)?;
        let output = match self {
            Self::Empty(vertex) => {
                *self = Self::Header(HeaderVertex {
                    importance: HeaderImportance::Imported,
                    parent: parent_id,
                    know_most: vertex.know_most.clone(),
                });
                Ok(true)
            }
            Self::Header(vertex) => match vertex.importance {
                HeaderImportance::Imported => Ok(false),
                _ => {
                    vertex.importance = HeaderImportance::Imported;
                    Ok(true)
                }
            },
            Self::Justification(vertex) => match vertex.importance {
                JustificationImportance::Imported => Ok(false),
                _ => {
                    vertex.importance = JustificationImportance::Imported;
                    Ok(true)
                }
            },
        };
        if let Some(holder) = holder {
            match self {
                Self::Empty(vertex) => vertex.know_most.insert(holder),
                Self::Header(vertex) => vertex.know_most.insert(holder),
                Self::Justification(_) => false,
            };
        }
        output
    }

    pub fn try_insert_justification(
        &mut self,
        justification_with_parent: JustificationWithParent<J>,
        holder: Option<I>,
    ) -> Result<bool, Error> {
        self.check_parent(&justification_with_parent.parent)?;
        match self {
            Self::Empty(_) | Self::Header(_) => {
                *self = Self::Justification(JustificationVertex {
                    importance: match self {
                        Self::Empty(vertex) => {
                            (&Into::<HeaderImportance>::into(&vertex.importance)).into()
                        }
                        Self::Header(vertex) => (&vertex.importance).into(),
                        Self::Justification(vertex) => Some(vertex.importance.clone()),
                    }
                    .ok_or(Error::JustificationImportance)?,
                    justification_with_parent,
                    know_most: HashSet::from_iter(holder.into_iter()),
                });
                Ok(true)
            }
            Self::Justification(_) => Ok(false),
        }
    }
}
