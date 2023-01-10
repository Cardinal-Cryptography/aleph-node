use std::collections::{
    hash_map::{Entry, OccupiedEntry, VacantEntry},
    HashMap, HashSet,
};

use vertex::{Error as VertexError, Importance, Vertex as Vertex_};

use super::{BlockIdentifier, Header, Justification, PeerID};

mod vertex;

type BlockIDFor<J> = <<J as Justification>::Header as Header>::Identifier;

pub struct JustificationWithParent<J: Justification> {
    pub justification: J,
    pub parent: BlockIDFor<J>,
}

impl<J: Justification> JustificationWithParent<J> {
    fn new(justification: J) -> Option<Self> {
        justification.header().parent_id().map(|id| Self {
            justification,
            parent: id,
        })
    }
}

pub enum VertexState<'a, I: PeerID, J: Justification> {
    HopelessFork,
    BelowMinimal,
    HighestFinalized,
    Unknown(VacantEntry<'a, BlockIDFor<J>, Vertex<I, J>>),
    Candidate(OccupiedEntry<'a, BlockIDFor<J>, Vertex<I, J>>),
}

pub enum Error {
    Vertex(VertexError),
    MissingVertex,
    ShouldBePruned,
    InfiniteLoop,
}

impl From<VertexError> for Error {
    fn from(err: VertexError) -> Self {
        Self::Vertex(err)
    }
}

pub struct Vertex<I: PeerID, J: Justification> {
    vertex: Vertex_<I, J>,
    children: HashSet<BlockIDFor<J>>,
}

impl<I: PeerID, J: Justification> Vertex<I, J> {
    fn new(holder: Option<I>) -> Self {
        Self {
            vertex: Vertex_::new(holder),
            children: HashSet::new(),
        }
    }
}

pub struct Forest<I: PeerID, J: Justification> {
    vertices: HashMap<BlockIDFor<J>, Vertex<I, J>>,
    root_id: BlockIDFor<J>,
    root_children: HashSet<BlockIDFor<J>>,
    compost_bin: HashSet<BlockIDFor<J>>,
}

impl<I: PeerID, J: Justification> Forest<I, J> {
    pub fn new(highest_justified: BlockIDFor<J>) -> Self {
        Self {
            vertices: HashMap::new(),
            root_id: highest_justified,
            root_children: HashSet::new(),
            compost_bin: HashSet::new(),
        }
    }

    fn get_mut(&mut self, id: &BlockIDFor<J>) -> VertexState<I, J> {
        use VertexState::*;
        if id == &self.root_id {
            HighestFinalized
        } else if id.number() <= self.root_id.number() {
            BelowMinimal
        } else if self.compost_bin.contains(id) {
            HopelessFork
        } else {
            match self.vertices.entry(id.clone()) {
                Entry::Occupied(entry) => Candidate(entry),
                Entry::Vacant(entry) => Unknown(entry),
            }
        }
    }

    fn prune(
        &mut self,
        id: BlockIDFor<J>,
        modified: &mut HashSet<BlockIDFor<J>>,
    ) -> Result<(), Error> {
        if !self.vertices.contains_key(&id) {
            return Ok(());
        }
        let cap = self.vertices.len();
        let mut to_be_pruned = Vec::with_capacity(cap);
        to_be_pruned.push(id);
        let mut index: usize = 0;
        while let Some(id) = to_be_pruned.get(index) {
            to_be_pruned.extend(
                self.vertices
                    .get(id)
                    .ok_or(Error::MissingVertex)?
                    .children
                    .iter()
                    .cloned(),
            );
            index += 1;
            if index > cap {
                return Err(Error::InfiniteLoop);
            }
        }
        for id in to_be_pruned.iter() {
            self.vertices.remove(id).ok_or(Error::MissingVertex)?;
        }
        self.compost_bin.extend(to_be_pruned.clone());
        modified.extend(to_be_pruned.into_iter());
        Ok(())
    }

    fn try_add_parent(
        &mut self,
        id: BlockIDFor<J>,
        modified: &mut HashSet<BlockIDFor<J>>,
    ) -> Result<(), Error> {
        use VertexState::*;
        if let Candidate(mut entry) = self.get_mut(&id) {
            let vertex = entry.get_mut();
            let importance = vertex.vertex.state().importance();
            if let Some(parent_id) = vertex.vertex.parent().cloned() {
                match self.get_mut(&parent_id) {
                    Unknown(_) => (),
                    HighestFinalized => {
                        self.root_children.insert(id);
                    }
                    Candidate(mut entry) => {
                        entry.get_mut().children.insert(id);
                        match importance {
                            Importance::Required | Importance::TopRequired => {
                                self.set_required_with_ancestors(&parent_id, modified)?
                            }
                            Importance::Auxiliary | Importance::Imported => (),
                        };
                    }
                    HopelessFork | BelowMinimal => self.prune(id, modified)?,
                };
            };
        };
        Ok(())
    }

    fn set_required_with_ancestors(
        &mut self,
        id: &BlockIDFor<J>,
        modified: &mut HashSet<BlockIDFor<J>>,
    ) -> Result<(), Error> {
        use VertexState::{Candidate, HighestFinalized};
        let mut guard = id.number() as i64 - self.root_id.number() as i64;
        let mut id = id.clone();
        let mut vertex;
        loop {
            // check if we reached the root
            match self.get_mut(&id) {
                Candidate(mut entry) => {
                    vertex = entry.get_mut();
                    // check if already required
                    match vertex.vertex.try_set_required() {
                        true => modified.insert(id.clone()),
                        false => break,
                    };
                    // check if has parent
                    id = match vertex.vertex.parent() {
                        Some(id) => id.clone(),
                        None => break,
                    };
                }
                HighestFinalized => break,
                _ => return Err(Error::ShouldBePruned),
            };
            // avoid infinite loop
            guard -= 1;
            if guard < 0 {
                return Err(Error::InfiniteLoop);
            }
        }
        Ok(())
    }

    fn bump_required(
        &mut self,
        id: &BlockIDFor<J>,
        modified: &mut HashSet<BlockIDFor<J>>,
    ) -> Result<(), Error> {
        use VertexState::*;
        if let Candidate(mut entry) = self.get_mut(id) {
            if entry.get_mut().vertex.try_set_top_required() {
                modified.insert(id.clone());
                if let Some(parent_id) = entry.get_mut().vertex.parent().cloned() {
                    self.set_required_with_ancestors(&parent_id, modified)?;
                }
            }
        }
        Ok(())
    }

    pub fn update_block_identifier(
        &mut self,
        id: BlockIDFor<J>,
        holder: Option<I>,
        required: bool,
    ) -> Result<HashSet<BlockIDFor<J>>, Error> {
        let mut modified = HashSet::new();
        match self.get_mut(&id) {
            VertexState::Unknown(entry) => {
                entry.insert(Vertex::new(holder));
                modified.insert(id.clone());
            }
            VertexState::Candidate(mut entry) => {
                entry.get_mut().vertex.add_empty_holder(holder);
            }
            _ => (),
        }
        if required {
            self.bump_required(&id, &mut modified)?;
        }
        Ok(modified)
    }

    // pub fn update_header(
    //     &mut self,
    //     header: &J::Header,
    //     holder: Option<I>,
    // ) -> Result<HashSet<BlockIDFor<J>>, Error> {
    //     use VertexState::Candidate;
    //     let id = header.id();
    //     let parent_id = header.parent_id().ok_or(Error::HeaderMissingParentID)?;
    //     let mut modified = self.update_block_identifier(parent_id, holder.clone())?;
    //     modified.extend(self.update_block_identifier(id.clone(), holder.clone())?);
    //     if let Candidate(vertex) = self.get_mut(&id) {
    //         let summary = vertex.try_insert_header(header, holder)?;
    //         self.process_transition(id, summary, &mut modified)?;
    //     }
    //     Ok(modified)
    // }

    // pub fn update_body(
    //     &mut self,
    //     header: &J::Header,
    //     holder: Option<I>,
    // ) -> Result<HashSet<BlockIDFor<J>>, Error> {
    //     use VertexState::*;
    //     let id = header.id();
    //     let mut modified = self.update_header(header, holder.clone())?;
    //     if let Candidate(vertex) = self.get_mut(&id) {
    //         let parent_id = vertex.parent().clone().ok_or(Error::MissingParent)?;
    //         match self.get_mut(&parent_id) {
    //             Unknown | HopelessFork | BelowMinimal => return Err(Error::MissingVertex),
    //             HighestFinalized => (),
    //             Candidate(parent_vertex) => {
    //                 if !parent_vertex.is_imported() {
    //                     return Err(Error::ParentShouldBeImported);
    //                 }
    //             }
    //         };
    //     }
    //     if let Candidate(vertex) = self.get_mut(&id) {
    //         let summary = vertex.try_insert_body(header, holder)?;
    //         self.process_transition(id, summary, &mut modified)?;
    //     }
    //     Ok(modified)
    // }

    // pub fn update_justification(
    //     &mut self,
    //     justification: J,
    //     holder: Option<I>,
    // ) -> Result<HashSet<BlockIDFor<J>>, Error> {
    //     use VertexState::Candidate;
    //     let header = justification.header();
    //     let id = header.id();
    //     let mut modified = self.update_header(header, holder.clone())?;
    //     if let Candidate(vertex) = self.get_mut(&id) {
    //         let summary = vertex.try_insert_justification(justification, holder)?;
    //         self.process_transition(id, summary, &mut modified)?;
    //     }
    //     Ok(modified)
    // }

    // fn find_next_trunk_vertex(
    //     &mut self,
    //     id: &BlockIDFor<J>,
    // ) -> Result<Option<BlockIDFor<J>>, Error> {
    //     let children = self.children.get(id).ok_or(Error::MissingChildrenHashSet)?;
    //     for child_id in children.clone().iter() {
    //         if let VertexState::Candidate(vertex) = self.get_mut(child_id) {
    //             if vertex.is_full()? {
    //                 return Ok(Some(child_id.clone()));
    //             }
    //         }
    //     }
    //     Ok(None)
    // }

    // #[allow(clippy::type_complexity)]
    // pub fn finalize(&mut self) -> Result<Option<Vec<(BlockIDFor<J>, J)>>, Error> {
    //     // find trunk
    //     let mut trunk = vec![];
    //     let mut id = self.root_id.clone();
    //     while let Some(child_id) = self.find_next_trunk_vertex(&id)? {
    //         trunk.push(child_id.clone());
    //         id = child_id;
    //     }
    //     // new root
    //     let new_root_id = match trunk.last() {
    //         Some(last) => last.clone(),
    //         None => return Ok(None),
    //     };
    //     // pruned branches don't have to be connected to the trunk!
    //     let to_be_pruned: HashSet<BlockIDFor<J>> = self
    //         .vertices
    //         .keys()
    //         .filter(|x| x.number() <= new_root_id.number())
    //         .cloned()
    //         .collect();
    //     for id in to_be_pruned.difference(&HashSet::from_iter(trunk.iter().cloned())) {
    //         if self.vertices.contains_key(id) {
    //             self.prune(id.clone())?;
    //         }
    //     }
    //     // keep new root children, as we'll remove the new root in a moment
    //     let new_root_children = self
    //         .children
    //         .get(&new_root_id)
    //         .ok_or(Error::MissingChildrenHashSet)?
    //         .clone();
    //     // remove trunk
    //     let mut finalized = vec![];
    //     for id in trunk.into_iter() {
    //         self.children
    //             .remove(&id)
    //             .ok_or(Error::MissingChildrenHashSet)?;
    //         match self
    //             .vertices
    //             .remove(&id)
    //             .ok_or(Error::MissingVertex)?
    //             .justification()
    //         {
    //             Some(justification) => finalized.push((id, justification)),
    //             None => return Err(Error::TrunkMissingJustification),
    //         };
    //     }
    //     // set new root
    //     self.root_id = new_root_id.clone();
    //     self.children.insert(new_root_id, new_root_children);
    //     // filter compost bin
    //     self.compost_bin = self
    //         .compost_bin
    //         .drain()
    //         .filter(|x| x.number() > self.root_id.number())
    //         .collect();
    //     Ok(Some(finalized))
    // }
}
