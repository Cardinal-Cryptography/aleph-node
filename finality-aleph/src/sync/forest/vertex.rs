use std::collections::HashSet;

use crate::sync::{forest::BlockIdFor, Justification, PeerId};

#[derive(Clone, Copy, PartialEq, Eq)]
enum HeaderImportance {
    Auxiliary,
    Required,
    Imported,
}

#[derive(Clone, PartialEq, Eq)]
enum InnerVertex<J: Justification> {
    /// Empty Vertex.
    Empty { required: bool },
    /// Vertex with added Header.
    Header {
        importance: HeaderImportance,
        parent: BlockIdFor<J>,
    },
    /// Vertex with added Header and Justification.
    Justification {
        imported: bool,
        justification: J,
        parent: BlockIdFor<J>,
    },
}

/// The vomplete vertex, including metadata about peers that know most about the data it refers to.
#[derive(Clone, PartialEq, Eq)]
pub struct Vertex<I: PeerId, J: Justification> {
    inner: InnerVertex<J>,
    know_most: HashSet<I>,
}

/// What can happen when we add a justification.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum JustificationAddResult {
    Noop,
    Required,
    Finalizable,
}

impl<I: PeerId, J: Justification> Vertex<I, J> {
    /// Create a new empty vertex.
    pub fn new() -> Self {
        Vertex {
            inner: InnerVertex::Empty { required: false },
            know_most: HashSet::new(),
        }
    }

    /// Whether the vertex is required.
    pub fn required(&self) -> bool {
        use InnerVertex::*;
        matches!(
            self.inner,
            Empty { required: true }
                | Header {
                    importance: HeaderImportance::Required,
                    ..
                }
                | Justification {
                    imported: false,
                    ..
                }
        )
    }

    /// Whether the vertex is imported.
    pub fn imported(&self) -> bool {
        use InnerVertex::*;
        matches!(
            self.inner,
            Header {
                importance: HeaderImportance::Imported,
                ..
            } | Justification { imported: true, .. }
        )
    }

    /// Deconstructs the vertex into a justification if it is ready to be imported,
    /// i.e. the related block has already been imported, otherwise returns it.
    pub fn ready(self) -> Result<J, Self> {
        match self.inner {
            InnerVertex::Justification {
                imported: true,
                justification,
                ..
            } => Ok(justification),
            _ => Err(self),
        }
    }

    /// The parent of the vertex, if known.
    pub fn parent(&self) -> Option<&BlockIdFor<J>> {
        match &self.inner {
            InnerVertex::Empty { .. } => None,
            InnerVertex::Header { parent, .. } => Some(parent),
            InnerVertex::Justification { parent, .. } => Some(parent),
        }
    }

    /// The list of peers which know most about the data this vertex refers to.
    pub fn know_most(&self) -> &HashSet<I> {
        &self.know_most
    }

    /// Set the vertex to be required, returns whether anything changed, i.e. the vertex was not
    /// required or imported before.
    pub fn set_required(&mut self) -> bool {
        use InnerVertex::*;
        match &self.inner {
            Empty { required: false } => {
                self.inner = Empty { required: true };
                true
            }
            Header {
                importance: HeaderImportance::Auxiliary,
                parent,
            } => {
                self.inner = Header {
                    importance: HeaderImportance::Required,
                    parent: parent.clone(),
                };
                true
            }
            _ => false,
        }
    }

    /// Adds a peer that knows most about the block this vertex refers to. Does nothing if we
    /// already have a justification.
    pub fn add_block_holder(&mut self, holder: Option<I>) {
        if let Some(holder) = holder {
            if !matches!(self.inner, InnerVertex::Justification { .. }) {
                self.know_most.insert(holder);
            }
        }
    }

    /// Adds the information the header provides to the vertex.
    pub fn insert_header(&mut self, parent: BlockIdFor<J>, holder: Option<I>) {
        self.add_block_holder(holder);
        if let InnerVertex::Empty { required } = self.inner {
            let importance = match required {
                false => HeaderImportance::Auxiliary,
                true => HeaderImportance::Required,
            };
            self.inner = InnerVertex::Header { importance, parent };
        }
    }

    /// Adds the information the header provides to the vertex and marks it as imported. Returns
    /// whether finalization is now possible.
    pub fn insert_body(&mut self, parent: BlockIdFor<J>) -> bool {
        use InnerVertex::*;
        match &self.inner {
            Empty { .. } | Header { .. } => {
                self.inner = Header {
                    parent,
                    importance: HeaderImportance::Imported,
                };
                false
            }
            Justification {
                imported: false,
                parent,
                justification,
            } => {
                self.inner = Justification {
                    imported: true,
                    parent: parent.clone(),
                    justification: justification.clone(),
                };
                true
            }
            _ => false,
        }
    }

    /// Adds a justification to the vertex. Returns whether either the finalization is now possible
    /// or the vertex became required.
    pub fn insert_justification(
        &mut self,
        parent: BlockIdFor<J>,
        justification: J,
        holder: Option<I>,
    ) -> JustificationAddResult {
        use InnerVertex::*;
        match self.inner {
            Justification { .. } => {
                if let Some(holder) = holder {
                    self.know_most.insert(holder);
                }
                JustificationAddResult::Noop
            }
            Empty { required: true }
            | Header {
                importance: HeaderImportance::Required,
                ..
            } => {
                self.inner = Justification {
                    imported: false,
                    parent,
                    justification,
                };
                self.know_most = holder.into_iter().collect();
                JustificationAddResult::Noop
            }
            Empty { required: false }
            | Header {
                importance: HeaderImportance::Auxiliary,
                ..
            } => {
                self.inner = Justification {
                    imported: false,
                    parent,
                    justification,
                };
                self.know_most = holder.into_iter().collect();
                JustificationAddResult::Required
            }
            Header {
                importance: HeaderImportance::Imported,
                ..
            } => {
                self.inner = Justification {
                    imported: true,
                    parent,
                    justification,
                };
                // No need to modify know_most, as we now know everything we need.
                JustificationAddResult::Finalizable
            }
        }
    }
}
