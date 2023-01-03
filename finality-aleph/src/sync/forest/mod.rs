use std::collections::{HashMap, HashSet};

use vertex::{Error as VertexError, Importance, TransitionSummary, Vertex};

use super::{BlockIdentifier, Header, Justification, PeerID};

mod vertex;

type BlockIdFor<J> = <<J as Justification>::Header as Header>::Identifier;

pub enum VertexState<'a, I: PeerID, J: Justification> {
    Unknown,
    HopelessFork,
    BelowMinimal,
    HighestFinalized,
    Candidate(&'a mut Vertex<I, J>),
}

pub enum RequestType {
    Header,
    Body,
    JustificationsBelow,
}

// /// TODO: RETHINK
// impl From<VertexState> for Option<RequestType> {
//     fn from(state: VertexState) -> Self {
//         use VertexState::*;
//         use Content::*;
//         use Importance::*;
//         use RequestType::{Header as RHeader, Body, JustificationsBelow};
//         match state {
//             Unknown | HopelessFork | BelowMinimal | HighestFinalized => None,
//             Candidate(Empty, Auxiliary) => Some(RHeader),
//             Candidate(Empty, TopRequired) => Some(Body),
//             Candidate(Empty, Required) => Some(Body),
//             Candidate(Empty, Imported) => {
//                 error!(target: "aleph-sync", "Forbidden state combination: (Empty, Imported), interpreting as (Header, Imported)");
//                 Some(JustificationsBelow)
//             },
//             Candidate(Header, Auxiliary) => None,
//             Candidate(Header, TopRequired) => Some(Body),
//             Candidate(Header, Required) => Some(Body),
//             Candidate(Header, Imported) => Some(JustificationsBelow),
//             Candidate(Justification, Auxiliary) => {
//                 error!(target: "aleph-sync", "Forbidden state combination: (Justification, Auxiliary), interpreting as (Justification, _)");
//                 Some(JustificationsBelow)
//             },
//             Candidate(Justification, _) => Some(JustificationsBelow),
//         }
//     }
// }

pub enum Error {
    Vertex(VertexError),
    MissingParent,
    MissingVertex,
    MissingChildrenHashSet,
    MissingJustification,
    CriticalBug,
    JustificationPruned,
    HeaderMissingParentID,
    ParentNotImported,
}

impl From<VertexError> for Error {
    fn from(err: VertexError) -> Self {
        Self::Vertex(err)
    }
}

pub struct Forest<I: PeerID, J: Justification> {
    vertices: HashMap<BlockIdFor<J>, Vertex<I, J>>,
    children: HashMap<BlockIdFor<J>, HashSet<BlockIdFor<J>>>,
    root_id: BlockIdFor<J>,
    compost_bin: HashSet<BlockIdFor<J>>,
}

impl<I: PeerID, J: Justification> Forest<I, J> {
    pub fn new(highest_justified: BlockIdFor<J>) -> Self {
        Self {
            vertices: HashMap::new(),
            children: HashMap::from([(highest_justified.clone(), HashSet::new())]),
            root_id: highest_justified,
            compost_bin: HashSet::new(),
        }
    }

    fn minimal_number(&self) -> u32 {
        self.root_id.number()
    }

    fn get_mut(&mut self, id: &BlockIdFor<J>) -> Result<VertexState<I, J>, Error> {
        use VertexState::*;
        if id == &self.root_id {
            Ok(HighestFinalized)
        } else if id.number() <= self.minimal_number() {
            Ok(BelowMinimal)
        } else if self.compost_bin.contains(id) {
            Ok(HopelessFork)
        } else {
            match self.vertices.get_mut(id) {
                Some(vertex) => Ok(Candidate(vertex)),
                None => Ok(Unknown),
            }
        }
    }

    fn add_holder(&mut self, id: BlockIdFor<J>, holder: Option<I>) -> Result<bool, Error> {
        Ok(match self.get_mut(&id)? {
            VertexState::Candidate(vertex) => vertex.add_holder(holder),
            _ => false,
        })
    }

    fn add_justification_holder(
        &mut self,
        id: BlockIdFor<J>,
        holder: Option<I>,
    ) -> Result<bool, Error> {
        Ok(match self.get_mut(&id)? {
            VertexState::Candidate(vertex) => vertex.add_justification_holder(holder),
            _ => false,
        })
    }

    fn insert_vertex(&mut self, id: BlockIdFor<J>) -> Result<Option<TransitionSummary>, Error> {
        use VertexState::*;
        Ok(match self.get_mut(&id)? {
            Unknown => {
                let (vertex, summary) = Vertex::new();
                if self.vertices.insert(id.clone(), vertex).is_some()
                    || self.children.insert(id, HashSet::new()).is_some()
                {
                    return Err(Error::CriticalBug);
                }
                Some(summary)
            }
            _ => None,
        })
    }

    fn remove_vertex(&mut self, id: &BlockIdFor<J>) -> Result<Option<J>, Error> {
        self.children
            .remove(id)
            .ok_or(Error::MissingChildrenHashSet)?;
        Ok(self
            .vertices
            .remove(id)
            .ok_or(Error::MissingVertex)?
            .justification())
    }

    fn try_bump_required_recursive(
        &mut self,
        id: &BlockIdFor<J>,
    ) -> Result<HashSet<BlockIdFor<J>>, Error> {
        use VertexState::{Candidate, HighestFinalized};
        let mut modified = HashSet::new();
        let mut guard = id.number() as i64 - self.minimal_number() as i64;
        if let Candidate(mut vertex) = self.get_mut(id)? {
            // if condition is false, then it's already required
            // we proceed nevertheless, because we might've just set the parent
            if vertex.try_set_top_required()?.is_some() {
                modified.insert(id.clone());
            }
            let mut id: BlockIdFor<J>;
            loop {
                // check if has parent
                id = match vertex.parent() {
                    Some(id) => id.clone(),
                    None => break,
                };
                // check if we reached the root
                vertex = match self.get_mut(&id)? {
                    Candidate(vertex) => vertex,
                    HighestFinalized => break,
                    _ => return Err(Error::CriticalBug),
                };
                // check if already required
                match vertex.try_set_required()? {
                    Some(_) => modified.insert(id.clone()),
                    None => break,
                };
                // avoid infinite loop
                guard -= 1;
                if guard < 0 {
                    return Err(Error::CriticalBug);
                }
            }
        }
        Ok(modified)
    }

    fn descendants(&self, id: &BlockIdFor<J>) -> Result<HashSet<BlockIdFor<J>>, Error> {
        let mut result = HashSet::new();
        let mut current = HashSet::from([id.clone()]);
        while !current.is_empty() {
            let mut next_current = HashSet::new();
            for current_id in current.into_iter() {
                next_current.extend(
                    self.children
                        .get(&current_id)
                        .ok_or(Error::MissingChildrenHashSet)?
                        .clone(),
                );
            }
            current = next_current;
            result.extend(current.clone());
        }
        Ok(result)
    }

    fn prune(&mut self, id: &BlockIdFor<J>) -> Result<HashSet<BlockIdFor<J>>, Error> {
        let mut to_be_pruned = self.descendants(id)?;
        to_be_pruned.insert(id.clone());
        for id in to_be_pruned.iter() {
            if self.remove_vertex(id)?.is_some() {
                return Err(Error::JustificationPruned);
            }
        }
        Ok(to_be_pruned)
    }

    fn process_transition(
        &mut self,
        id: BlockIdFor<J>,
        summary: Option<TransitionSummary>,
        modified: &mut HashSet<BlockIdFor<J>>,
    ) -> Result<(), Error> {
        use Importance::*;
        use VertexState::*;
        if let Some(summary) = summary {
            modified.insert(id.clone());
            if summary.gained_parent {
                if let Candidate(vertex) = self.get_mut(&id)? {
                    let parent_id = vertex.parent().clone().ok_or(Error::MissingParent)?;
                    match self.get_mut(&parent_id)? {
                        Unknown => return Err(Error::MissingParent),
                        HighestFinalized | Candidate(_) => self
                            .children
                            .get_mut(&parent_id)
                            .ok_or(Error::MissingChildrenHashSet)?
                            .insert(id.clone()),
                        HopelessFork | BelowMinimal => {
                            modified.extend(self.prune(&id)?);
                            return Ok(());
                        }
                    };
                };
                if let Candidate(vertex) = self.get_mut(&id)? {
                    let (_, importance) = vertex.state()?;
                    match importance {
                        Required | TopRequired => {
                            modified.extend(self.try_bump_required_recursive(&id)?)
                        }
                        Auxiliary | Imported => (),
                    }
                }
            }
        }
        Ok(())
    }

    pub fn set_required(&mut self, id: &BlockIdFor<J>) -> Result<HashSet<BlockIdFor<J>>, Error> {
        self.try_bump_required_recursive(id)
    }

    pub fn update_block_identifier(
        &mut self,
        id: BlockIdFor<J>,
        holder: Option<I>,
    ) -> Result<HashSet<BlockIdFor<J>>, Error> {
        let mut modified = HashSet::new();
        let summary = self.insert_vertex(id.clone())?;
        self.process_transition(id.clone(), summary, &mut modified)?;
        self.add_holder(id, holder)?;
        Ok(modified)
    }

    pub fn update_header(
        &mut self,
        header: &J::Header,
        holder: Option<I>,
    ) -> Result<HashSet<BlockIdFor<J>>, Error> {
        use VertexState::Candidate;
        let id = header.id();
        let parent_id = header.parent_id().ok_or(Error::HeaderMissingParentID)?;
        let mut modified = self.update_block_identifier(parent_id, holder.clone())?;
        modified.extend(self.update_block_identifier(id.clone(), holder)?);
        if let Candidate(vertex) = self.get_mut(&id)? {
            let summary = vertex.try_insert_header(header)?;
            self.process_transition(id, summary, &mut modified)?;
        }
        Ok(modified)
    }

    pub fn update_body(
        &mut self,
        header: &J::Header,
        holder: Option<I>,
    ) -> Result<HashSet<BlockIdFor<J>>, Error> {
        use VertexState::*;
        let id = header.id();
        let mut modified = self.update_header(header, holder)?;
        if let Candidate(vertex) = self.get_mut(&id)? {
            let parent_id = vertex.parent().clone().ok_or(Error::MissingParent)?;
            match self.get_mut(&parent_id)? {
                Unknown | HopelessFork | BelowMinimal => return Err(Error::MissingVertex),
                HighestFinalized => (),
                Candidate(parent_vertex) => {
                    if !parent_vertex.is_imported() {
                        return Err(Error::ParentNotImported);
                    }
                }
            };
        }
        if let Candidate(vertex) = self.get_mut(&id)? {
            let summary = vertex.try_insert_body(header)?;
            self.process_transition(id, summary, &mut modified)?;
        }
        Ok(modified)
    }

    pub fn update_justification(
        &mut self,
        justification: J,
        holder: Option<I>,
    ) -> Result<HashSet<BlockIdFor<J>>, Error> {
        use VertexState::Candidate;
        let header = justification.header();
        let id = header.id();
        let mut modified = self.update_header(header, holder.clone())?;
        if let Candidate(vertex) = self.get_mut(&id)? {
            let summary = vertex.try_insert_justification(justification)?;
            vertex.add_justification_holder(holder);
            self.process_transition(id, summary, &mut modified)?;
        }
        Ok(modified)
    }

    fn find_full_child(&mut self, id: &BlockIdFor<J>) -> Result<Option<BlockIdFor<J>>, Error> {
        let children = self.children.get(id).ok_or(Error::MissingChildrenHashSet)?;
        for child_id in children.clone().iter() {
            if let VertexState::Candidate(vertex) = self.get_mut(child_id)? {
                if vertex.is_full()? {
                    return Ok(Some(child_id.clone()));
                }
            }
        }
        Ok(None)
    }

    fn find_trunk(&mut self) -> Result<Vec<BlockIdFor<J>>, Error> {
        let mut trunk = vec![];
        let mut id = self.root_id.clone();
        while let Some(child_id) = self.find_full_child(&id)? {
            trunk.push(child_id.clone());
            id = child_id;
        }
        Ok(trunk)
    }

    #[allow(clippy::type_complexity)]
    pub fn finalize(&mut self) -> Result<Option<Vec<(BlockIdFor<J>, J)>>, Error> {
        let trunk = self.find_trunk()?;
        let new_root_id = match trunk.last() {
            Some(last) => last.clone(),
            None => return Ok(None),
        };
        // pruned branches don't have to be connected to the trunk!
        let to_be_pruned: HashSet<BlockIdFor<J>> = self
            .vertices
            .keys()
            .filter(|x| x.number() <= new_root_id.number())
            .cloned()
            .collect();
        for id in to_be_pruned.difference(&HashSet::from_iter(trunk.iter().cloned())) {
            if self.vertices.contains_key(id) {
                self.prune(id)?;
            }
        }
        let new_root_children = self
            .children
            .get(&new_root_id)
            .ok_or(Error::MissingChildrenHashSet)?
            .clone();
        let mut finalized = vec![];
        for id in trunk.into_iter() {
            match self.remove_vertex(&id)? {
                Some(justification) => finalized.push((id, justification)),
                None => return Err(Error::MissingJustification),
            };
        }
        self.root_id = new_root_id.clone();
        self.children.insert(new_root_id, new_root_children);
        let minimal_number = self.minimal_number();
        self.compost_bin = self
            .compost_bin
            .drain()
            .filter(|x| x.number() > minimal_number)
            .collect();
        Ok(Some(finalized))
    }
}
