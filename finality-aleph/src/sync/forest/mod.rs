use std::collections::{
    hash_map::{Entry, OccupiedEntry, VacantEntry},
    HashMap, HashSet,
};

use vertex::{Error as VertexError, Importance, State, Vertex as Vertex_};

use super::{BlockIdentifier, Header, Justification, PeerID};

mod vertex;

type BlockIDFor<J> = <<J as Justification>::Header as Header>::Identifier;

pub struct JustificationWithParent<J: Justification> {
    pub justification: J,
    pub parent: BlockIDFor<J>,
}

enum VertexHandle<'a, I: PeerID, J: Justification> {
    HopelessFork,
    BelowMinimal,
    HighestFinalized,
    Unknown(VacantEntry<'a, BlockIDFor<J>, Vertex<I, J>>),
    Candidate(OccupiedEntry<'a, BlockIDFor<J>, Vertex<I, J>>),
}

pub enum VertexState<I: PeerID> {
    HopelessFork,
    BelowMinimal,
    HighestFinalized,
    Unknown,
    Candidate(State, HashSet<I>),
}

impl<'a, I: PeerID, J: Justification> From<VertexHandle<'a, I, J>> for VertexState<I> {
    fn from(handle: VertexHandle<'a, I, J>) -> Self {
        match handle {
            VertexHandle::HopelessFork => Self::HopelessFork,
            VertexHandle::BelowMinimal => Self::BelowMinimal,
            VertexHandle::HighestFinalized => Self::HighestFinalized,
            VertexHandle::Unknown(_) => Self::Unknown,
            VertexHandle::Candidate(entry) => Self::Candidate(
                entry.get().vertex.state(),
                entry.get().vertex.know_most().clone(),
            ),
        }
    }
}

pub enum Critical {
    Vertex(VertexError),
    MissingVertex,
    ShouldBePruned,
    InfiniteLoop,
}

pub enum Error {
    Critical(Critical),
    HeaderMissingParentID,
    IncorrectParentState,
    ParentNotImported,
}

impl From<VertexError> for Error {
    fn from(err: VertexError) -> Self {
        Self::Critical(Critical::Vertex(err))
    }
}

impl From<Critical> for Error {
    fn from(err: Critical) -> Self {
        Self::Critical(err)
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

    fn get_mut(&mut self, id: &BlockIDFor<J>) -> VertexHandle<I, J> {
        use VertexHandle::*;
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
        id: &BlockIDFor<J>,
        modified: &mut HashSet<BlockIDFor<J>>,
    ) -> Result<(), Error> {
        if !self.vertices.contains_key(id) {
            return Ok(());
        }
        let cap = self.vertices.len();
        let mut to_be_pruned = Vec::with_capacity(cap);
        to_be_pruned.push(id.clone());
        let mut index: usize = 0;
        while let Some(id) = to_be_pruned.get(index) {
            to_be_pruned.extend(
                self.vertices
                    .get(id)
                    .ok_or(Critical::MissingVertex)?
                    .children
                    .iter()
                    .cloned(),
            );
            index += 1;
            if index > cap {
                Err(Critical::InfiniteLoop)?;
            }
        }
        for id in to_be_pruned.iter() {
            self.vertices.remove(id).ok_or(Critical::MissingVertex)?;
        }
        self.compost_bin.extend(to_be_pruned.clone());
        modified.extend(to_be_pruned.into_iter());
        Ok(())
    }

    fn try_add_parent(
        &mut self,
        id: &BlockIDFor<J>,
        modified: &mut HashSet<BlockIDFor<J>>,
    ) -> Result<(), Error> {
        use VertexHandle::*;
        if let Candidate(mut entry) = self.get_mut(id) {
            let vertex = entry.get_mut();
            let importance = vertex.vertex.state().importance();
            if let Some(parent_id) = vertex.vertex.parent().cloned() {
                match self.get_mut(&parent_id) {
                    Unknown(_) => (),
                    HighestFinalized => {
                        self.root_children.insert(id.clone());
                    }
                    Candidate(mut entry) => {
                        entry.get_mut().children.insert(id.clone());
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
        use VertexHandle::{Candidate, HighestFinalized};
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
                _ => Err(Critical::ShouldBePruned)?,
            };
            // avoid infinite loop
            guard -= 1;
            if guard < 0 {
                Err(Critical::InfiniteLoop)?;
            }
        }
        Ok(())
    }

    fn bump_required(
        &mut self,
        id: &BlockIDFor<J>,
        modified: &mut HashSet<BlockIDFor<J>>,
    ) -> Result<(), Error> {
        use VertexHandle::*;
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

    fn bump_vertex(
        &mut self,
        id: &BlockIDFor<J>,
        holder: Option<I>,
        modified: &mut HashSet<BlockIDFor<J>>,
    ) {
        match self.get_mut(id) {
            VertexHandle::Unknown(entry) => {
                entry.insert(Vertex::new(holder));
                modified.insert(id.clone());
            }
            VertexHandle::Candidate(mut entry) => {
                entry.get_mut().vertex.add_block_id_holder(holder);
            }
            _ => (),
        }
    }

    fn process_header(
        &mut self,
        header: &J::Header,
    ) -> Result<(BlockIDFor<J>, BlockIDFor<J>), Error> {
        Ok((
            header.id(),
            header.parent_id().ok_or(Error::HeaderMissingParentID)?,
        ))
    }

    fn process_justification(
        &mut self,
        justification: J,
    ) -> Result<(BlockIDFor<J>, JustificationWithParent<J>), Error> {
        let (id, parent) = self.process_header(justification.header())?;
        Ok((
            id,
            JustificationWithParent {
                justification,
                parent,
            },
        ))
    }

    pub fn update_block_identifier(
        &mut self,
        id: BlockIDFor<J>,
        holder: Option<I>,
        required: bool,
    ) -> Result<HashSet<BlockIDFor<J>>, Error> {
        let mut modified = HashSet::new();
        self.bump_vertex(&id, holder.clone(), &mut modified);
        if required {
            self.bump_required(&id, &mut modified)?;
        }
        if let VertexHandle::Candidate(mut entry) = self.get_mut(&id) {
            entry.get_mut().vertex.add_block_id_holder(holder);
        }
        Ok(modified)
    }

    pub fn update_header(
        &mut self,
        header: &J::Header,
        holder: Option<I>,
        required: bool,
    ) -> Result<HashSet<BlockIDFor<J>>, Error> {
        let mut modified = HashSet::new();
        let (id, parent_id) = self.process_header(header)?;
        self.bump_vertex(&parent_id, holder.clone(), &mut modified);
        self.bump_vertex(&id, holder.clone(), &mut modified);
        if let VertexHandle::Candidate(mut entry) = self.get_mut(&id) {
            if entry
                .get_mut()
                .vertex
                .try_insert_header(parent_id.clone(), holder)?
            {
                modified.insert(id.clone());
            }
            self.try_add_parent(&id, &mut modified)?;
        }
        if required {
            self.bump_required(&id, &mut modified)?;
        }
        Ok(modified)
    }

    pub fn update_body(
        &mut self,
        header: &J::Header,
        holder: Option<I>,
    ) -> Result<HashSet<BlockIDFor<J>>, Error> {
        use VertexHandle::*;
        let mut modified = HashSet::new();
        let (id, parent_id) = self.process_header(header)?;
        self.bump_vertex(&parent_id, holder.clone(), &mut modified);
        self.bump_vertex(&id, holder.clone(), &mut modified);
        match self.get_mut(&parent_id) {
            Candidate(entry) => {
                if !entry.get().vertex.is_imported() {
                    Err(Error::ParentNotImported)?;
                }
            }
            HighestFinalized => (),
            Unknown(_) | HopelessFork | BelowMinimal => Err(Error::IncorrectParentState)?,
        }
        if let VertexHandle::Candidate(mut entry) = self.get_mut(&id) {
            if entry
                .get_mut()
                .vertex
                .try_insert_body(parent_id.clone(), holder)?
            {
                modified.insert(id.clone());
            }
            self.try_add_parent(&id, &mut modified)?;
        }
        Ok(modified)
    }

    pub fn update_justification(
        &mut self,
        justification: J,
        holder: Option<I>,
    ) -> Result<HashSet<BlockIDFor<J>>, Error> {
        let mut modified = HashSet::new();
        let (id, justification_with_parent) = self.process_justification(justification)?;
        self.bump_vertex(
            &justification_with_parent.parent,
            holder.clone(),
            &mut modified,
        );
        self.bump_vertex(&id, holder.clone(), &mut modified);
        self.bump_required(&id, &mut modified)?;
        if let VertexHandle::Candidate(mut entry) = self.get_mut(&id) {
            if entry
                .get_mut()
                .vertex
                .try_insert_justification(justification_with_parent, holder)?
            {
                modified.insert(id.clone());
            }
            self.try_add_parent(&id, &mut modified)?;
        }
        Ok(modified)
    }

    #[allow(clippy::type_complexity)]
    pub fn try_finalize(&mut self) -> Result<Option<(J, HashSet<BlockIDFor<J>>)>, Error> {
        let mut modified = HashSet::new();
        for child_id in self.root_children.clone().iter() {
            if let VertexHandle::Candidate(entry) = self.get_mut(child_id) {
                if let Some(justification) = entry.get().vertex.is_full() {
                    let (new_root_id, vertex) = entry.remove_entry();
                    modified.insert(new_root_id.clone());
                    self.root_id = new_root_id;
                    self.root_children = vertex.children;
                    let to_be_pruned: Vec<BlockIDFor<J>> = self
                        .vertices
                        .keys()
                        .filter(|k| k.number() <= self.root_id.number())
                        .cloned()
                        .collect();
                    for id in to_be_pruned.iter() {
                        self.prune(id, &mut modified)?;
                    }
                    self.compost_bin = self
                        .compost_bin
                        .drain()
                        .filter(|x| x.number() > self.root_id.number())
                        .collect();
                    return Ok(Some((justification, modified)));
                }
            }
        }
        Ok(None)
    }

    pub fn state(&mut self, id: &BlockIDFor<J>) -> VertexState<I> {
        self.get_mut(id).into()
    }
}
