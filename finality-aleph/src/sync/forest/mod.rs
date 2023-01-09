use std::collections::{HashMap, HashSet};

use vertex::{Error as VertexError, Importance, TransitionSummary, Vertex};

use super::{BlockIdentifier, Header, Justification, PeerID};

mod vertex;

type BlockIdFor<J> = <<J as Justification>::Header as Header>::Identifier;

struct JustificationWithParent<J: Justification> {
    pub justification: J,
    pub parent: BlockIdFor<J>,
}

impl<J: Justification> JustificationWithParent<J> {
    fn new(justification: J) -> Option<Self> {
        justification.header().parent_id().map(|id| Self { justification, parent: id.clone() })
    }
}


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


pub enum Error {
    Vertex(VertexError),
    MissingParent,
    MissingVertex,
    MissingChildrenHashSet,
    TrunkMissingJustification,
    RootPruned,
    UnknownIDPresent,
    ShouldBePruned,
    InfiniteLoop,
    HeaderMissingParentID,
    ParentShouldBeImported,
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

    fn get_mut(&mut self, id: &BlockIdFor<J>) -> VertexState<I, J> {
        use VertexState::*;
        if id == &self.root_id {
            HighestFinalized
        } else if id.number() <= self.root_id.number() {
            BelowMinimal
        } else if self.compost_bin.contains(id) {
            HopelessFork
        } else {
            match self.vertices.get_mut(id) {
                Some(vertex) => Candidate(vertex),
                None => Unknown,
            }
        }
    }

    fn prune(&mut self, id: BlockIdFor<J>) -> Result<HashSet<BlockIdFor<J>>, Error> {
        if let VertexState::HighestFinalized = self.get_mut(&id) {
            return Err(Error::RootPruned);
        }
        let mut to_be_pruned: HashSet<BlockIdFor<J>> = HashSet::new();
        let mut current = HashSet::from([id]);
        let mut guard = self.vertices.len() as i64;
        while !current.is_empty() {
            to_be_pruned.extend(current.clone());
            let mut next_current = HashSet::new();
            for current_id in current.iter() {
                next_current.extend(
                    self.children
                        .get(current_id)
                        .ok_or(Error::MissingChildrenHashSet)?
                        .clone(),
                );
            }
            current = next_current;
            // avoid infinite loop
            if guard < 0 {
                return Err(Error::InfiniteLoop);
            }
            guard -= 1;
        }
        for id in to_be_pruned.iter() {
            self.children
                .remove(id)
                .ok_or(Error::MissingChildrenHashSet)?;
            self.vertices.remove(id).ok_or(Error::MissingVertex)?;
        }
        self.compost_bin.extend(to_be_pruned.clone());
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
                if let Candidate(vertex) = self.get_mut(&id) {
                    let parent_id = vertex.parent().clone().ok_or(Error::MissingParent)?;
                    match self.get_mut(&parent_id) {
                        Unknown => return Err(Error::MissingParent),
                        HighestFinalized | Candidate(_) => self
                            .children
                            .get_mut(&parent_id)
                            .ok_or(Error::MissingChildrenHashSet)?
                            .insert(id.clone()),
                        HopelessFork | BelowMinimal => {
                            modified.extend(self.prune(id)?);
                            return Ok(());
                        }
                    };
                };
                if let Candidate(vertex) = self.get_mut(&id) {
                    let (_, importance) = vertex.state()?;
                    match importance {
                        Required | TopRequired => modified.extend(self.set_required(&id)?),
                        Auxiliary | Imported => (),
                    }
                }
            }
        }
        Ok(())
    }

    pub fn set_required(&mut self, id: &BlockIdFor<J>) -> Result<HashSet<BlockIdFor<J>>, Error> {
        use VertexState::{Candidate, HighestFinalized};
        let mut modified = HashSet::new();
        let mut guard = id.number() as i64 - self.root_id.number() as i64;
        if let Candidate(mut vertex) = self.get_mut(id) {
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
                vertex = match self.get_mut(&id) {
                    Candidate(vertex) => vertex,
                    HighestFinalized => break,
                    _ => return Err(Error::ShouldBePruned),
                };
                // check if already required
                match vertex.try_set_required()? {
                    Some(_) => modified.insert(id.clone()),
                    None => break,
                };
                // avoid infinite loop
                guard -= 1;
                if guard < 0 {
                    return Err(Error::InfiniteLoop);
                }
            }
        }
        Ok(modified)
    }

    pub fn update_block_identifier(
        &mut self,
        id: BlockIdFor<J>,
        holder: Option<I>,
    ) -> Result<HashSet<BlockIdFor<J>>, Error> {
        let mut modified = HashSet::new();
        // insert vertex
        let summary = match self.get_mut(&id) {
            VertexState::Unknown => {
                let (vertex, summary) = Vertex::new(holder);
                if self.vertices.insert(id.clone(), vertex).is_some()
                    || self.children.insert(id.clone(), HashSet::new()).is_some()
                {
                    return Err(Error::UnknownIDPresent);
                }
                Some(summary)
            }
            _ => None,
        };
        self.process_transition(id, summary, &mut modified)?;
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
        modified.extend(self.update_block_identifier(id.clone(), holder.clone())?);
        if let Candidate(vertex) = self.get_mut(&id) {
            let summary = vertex.try_insert_header(header, holder)?;
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
        let mut modified = self.update_header(header, holder.clone())?;
        if let Candidate(vertex) = self.get_mut(&id) {
            let parent_id = vertex.parent().clone().ok_or(Error::MissingParent)?;
            match self.get_mut(&parent_id) {
                Unknown | HopelessFork | BelowMinimal => return Err(Error::MissingVertex),
                HighestFinalized => (),
                Candidate(parent_vertex) => {
                    if !parent_vertex.is_imported() {
                        return Err(Error::ParentShouldBeImported);
                    }
                }
            };
        }
        if let Candidate(vertex) = self.get_mut(&id) {
            let summary = vertex.try_insert_body(header, holder)?;
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
        if let Candidate(vertex) = self.get_mut(&id) {
            let summary = vertex.try_insert_justification(justification, holder)?;
            self.process_transition(id, summary, &mut modified)?;
        }
        Ok(modified)
    }

    fn find_next_trunk_vertex(
        &mut self,
        id: &BlockIdFor<J>,
    ) -> Result<Option<BlockIdFor<J>>, Error> {
        let children = self.children.get(id).ok_or(Error::MissingChildrenHashSet)?;
        for child_id in children.clone().iter() {
            if let VertexState::Candidate(vertex) = self.get_mut(child_id) {
                if vertex.is_full()? {
                    return Ok(Some(child_id.clone()));
                }
            }
        }
        Ok(None)
    }

    #[allow(clippy::type_complexity)]
    pub fn finalize(&mut self) -> Result<Option<Vec<(BlockIdFor<J>, J)>>, Error> {
        // find trunk
        let mut trunk = vec![];
        let mut id = self.root_id.clone();
        while let Some(child_id) = self.find_next_trunk_vertex(&id)? {
            trunk.push(child_id.clone());
            id = child_id;
        }
        // new root
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
                self.prune(id.clone())?;
            }
        }
        // keep new root children, as we'll remove the new root in a moment
        let new_root_children = self
            .children
            .get(&new_root_id)
            .ok_or(Error::MissingChildrenHashSet)?
            .clone();
        // remove trunk
        let mut finalized = vec![];
        for id in trunk.into_iter() {
            self.children
                .remove(&id)
                .ok_or(Error::MissingChildrenHashSet)?;
            match self
                .vertices
                .remove(&id)
                .ok_or(Error::MissingVertex)?
                .justification()
            {
                Some(justification) => finalized.push((id, justification)),
                None => return Err(Error::TrunkMissingJustification),
            };
        }
        // set new root
        self.root_id = new_root_id.clone();
        self.children.insert(new_root_id, new_root_children);
        // filter compost bin
        self.compost_bin = self
            .compost_bin
            .drain()
            .filter(|x| x.number() > self.root_id.number())
            .collect();
        Ok(Some(finalized))
    }

    // pub fn sth_request(...) {}
}
