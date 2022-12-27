use std::collections::HashSet;

pub mod graph;
use graph::{Error, Forest as Graph};

#[derive(Clone, std::cmp::PartialEq, std::cmp::Eq, std::hash::Hash)]
pub struct Hash;

#[derive(Clone, std::cmp::PartialEq, std::cmp::Eq, std::hash::Hash)]
pub struct HashNumber {
    hash: Hash,
    number: u32,
}

#[derive(Clone, std::cmp::PartialEq, std::cmp::Eq, std::hash::Hash)]
pub struct PeerID;

#[derive(Clone)]
pub struct Justification;

struct Vertex {
    body_imported: bool,
    justification: Option<Justification>,
    know_most: HashSet<PeerID>,
    required: bool,
}

impl Vertex {
    fn new(holder: Option<PeerID>, required: bool) -> Self {
        let know_most = match holder {
            Some(peer_id) => HashSet::from([peer_id]),
            None => HashSet::new(),
        };
        Vertex {
            body_imported: false,
            justification: None,
            know_most,
            required,
        }
    }

    fn add_holder(&mut self, holder: Option<PeerID>) {
        if let Some(peer_id) = holder {
            self.know_most.insert(peer_id);
        };
    }
}

#[derive(std::cmp::PartialEq, std::cmp::Eq)]
enum State {
    Unknown,
    Empty,
    EmptyRequired,
    Header,
    HeaderRequired,
    Body,
    JustifiedHeader,
    Full,
    HopelessFork,
    BelowMinimal,
}

pub enum RequestType {
    Header,
    Body,
    JustificationsBelow,
}

/// TODO: RETHINK
impl From<State> for Option<RequestType> {
    fn from(state: State) -> Self {
        use State::*;
        match state {
            Unknown | HopelessFork | BelowMinimal => None,
            Empty => Some(RequestType::Header),
            EmptyRequired | HeaderRequired | Header => Some(RequestType::Body),
            Body | JustifiedHeader | Full => Some(RequestType::JustificationsBelow),
        }
    }
}

pub struct Forest {
    graph: Graph<HashNumber, Vertex>,
    compost_bin: HashSet<HashNumber>,
}

impl Forest {
    pub fn new(highest_justified: HashNumber) -> Self {
        Self {
            graph: Graph::new(highest_justified),
            compost_bin: HashSet::new(),
        }
    }

    fn minimal_number(&self) -> u32 {
        self.graph.get_root().number
    }

    fn state(&self, hashnumber: &HashNumber) -> State {
        // Check if below the current lower bound.
        if hashnumber.number <= self.minimal_number() {
            return State::BelowMinimal;
        }
        // Check if it's a hopeless fork.
        if self.compost_bin.contains(hashnumber) {
            return State::HopelessFork;
        }
        // Check if we know it.
        let vertex = match self.graph.get(hashnumber) {
            Some(v) => v,
            None => return State::Unknown,
        };
        // Check if we don't know the parent, thus we haven't received the header.
        if self.graph.get_parent_key(hashnumber).is_none() {
            return match vertex.required {
                true => State::EmptyRequired,
                false => State::Empty,
            };
        };
        // Check the content: body and justification.
        match (&vertex.justification, vertex.body_imported) {
            (Some(_), true) => State::Full,
            (Some(_), false) => State::JustifiedHeader,
            (None, true) => State::Body,
            (None, false) => match vertex.required {
                true => State::HeaderRequired,
                false => State::Header,
            },
        }
    }

    /// Bumps flag `required` of the vertex and all its ancestors.
    fn bump_required(&mut self, hashnumber: &HashNumber) -> Result<HashSet<HashNumber>, Error> {
        let mut modified = HashSet::new();
        let mut hashnumber = hashnumber.clone();
        let mut old_state = self.state(&hashnumber);
        let mut vertex = match self.graph.get_mut(&hashnumber) {
            Some(v) => v,
            None => return Err(Error::MissingKey),
        };
        loop {
            if vertex.required {
                break;
            };
            vertex.required = true;
            if old_state != self.state(&hashnumber) {
                modified.insert(hashnumber.clone());
            };
            hashnumber = match self.graph.get_parent_key(&hashnumber) {
                Some(k) => {
                    if k == self.graph.get_root() {
                        break;
                    };
                    k.clone()
                }
                None => break,
            };
            old_state = self.state(&hashnumber);
            vertex = match self.graph.get_mut(&hashnumber) {
                Some(v) => v,
                None => return Err(Error::CriticalBug),
            };
        }
        Ok(modified)
    }

    pub fn update_hashnumber(
        &mut self,
        hashnumber: HashNumber,
        holder: Option<PeerID>,
        bump_required: bool,
    ) -> Result<HashSet<HashNumber>, Error> {
        use State::*;
        match self.state(&hashnumber) {
            // skip if the vertex is irrelevant, or we have a justification,
            // thus the information about the holder is unrelated, and the vertex
            // is required "by default"
            HopelessFork | BelowMinimal | JustifiedHeader | Full => Ok(HashSet::new()),
            // create the vertex if unknown to us
            Unknown => {
                self.graph
                    .insert(hashnumber.clone(), Vertex::new(holder, bump_required), None)?;
                Ok(HashSet::from([hashnumber]))
            }
            // update the vertex content
            Empty | EmptyRequired | Header | HeaderRequired | Body => {
                // add holder
                if let Some(peer_id) = holder {
                    match self.graph.get_mut(&hashnumber) {
                        Some(vertex) => vertex.know_most.insert(peer_id),
                        // we know the vertex
                        None => return Err(Error::CriticalBug),
                    };
                };
                // bump required - all ancestors
                match bump_required {
                    true => self.bump_required(&hashnumber),
                    false => Ok(HashSet::new()),
                }
            }
        }
    }

    pub fn update_header(
        &mut self,
        hashnumber: HashNumber,
        parent_hashnumber: HashNumber,
        holder: Option<PeerID>,
        bump_required: bool,
    ) -> Result<HashSet<HashNumber>, Error> {
        use State::*;
        let mut modified =
            self.update_hashnumber(hashnumber.clone(), holder.clone(), bump_required)?;
        modified.extend(match self.state(&hashnumber) {
            // skip if the vertex is irrelevant, or we have a justification,
            // thus the information about the holder is unrelated, and the vertex
            // is required "by default"
            HopelessFork | BelowMinimal | JustifiedHeader | Full => HashSet::new(),
            // we've just updated the hashnumber
            Unknown => return Err(Error::CriticalBug),
            // we already have the header
            Header | HeaderRequired | Body => {
                self.graph
                    .get_mut(&hashnumber)
                    .ok_or(Error::CriticalBug)?
                    .add_holder(holder);
                HashSet::new()
            }
            // this is the first time we got the header, thus the parent is not set
            Empty | EmptyRequired => {
                let mut modified = self.update_hashnumber(
                    parent_hashnumber.clone(),
                    holder.clone(),
                    bump_required,
                )?;
                // modify hashnumber vertex - add parent (we've already called `update_hashnumber`,
                // therefore we don't need to use `holder` and `bump_required` here
                self.graph
                    .set_parent(hashnumber.clone(), parent_hashnumber.clone())?;
                modified.insert(hashnumber.clone());
                match self.state(&parent_hashnumber) {
                    Unknown => return Err(Error::CriticalBug),
                    HopelessFork | BelowMinimal => {
                        self.compost_bin.extend(self.graph.prune(hashnumber)?);
                    }
                    Empty | EmptyRequired | Header | HeaderRequired | Body | JustifiedHeader
                    | Full => (),
                };
                modified
            }
        });
        Ok(modified)
    }

    pub fn update_header_and_justification(
        &mut self,
        hashnumber: HashNumber,
        parent_hashnumber: HashNumber,
        justification: Justification,
        holder: Option<PeerID>,
    ) -> Result<HashSet<HashNumber>, Error> {
        use State::*;
        let mut modified =
            self.update_header(hashnumber.clone(), parent_hashnumber, holder.clone(), false)?;
        modified.extend(match self.state(&hashnumber) {
            // skip if the vertex is irrelevant
            BelowMinimal => HashSet::new(),
            // we've just updated the hashnumber, added header, and justified vertex cannot be a HopelessFork
            Unknown | Empty | EmptyRequired | HopelessFork => return Err(Error::CriticalBug),
            // we already have the justification
            JustifiedHeader | Full => {
                self.graph
                    .get_mut(&hashnumber)
                    .ok_or(Error::CriticalBug)?
                    .add_holder(holder);
                HashSet::new()
            }
            // this is the first time we got the justification
            Header | HeaderRequired | Body => {
                let vertex = self.graph.get_mut(&hashnumber).ok_or(Error::CriticalBug)?;
                vertex.know_most = HashSet::new();
                vertex.add_holder(holder);
                vertex.justification = Some(justification);
                HashSet::from([hashnumber])
            }
        });
        Ok(modified)
    }

    fn find_trunk_top(&self) -> Result<HashNumber, Error> {
        let mut top = self.graph.get_root().clone();
        'outer: loop {
            for child in self
                .graph
                .get_children_keys(&top)
                .ok_or(Error::CriticalBug)?
                .iter()
            {
                if self.state(child) == State::Full {
                    top = child.clone();
                    continue 'outer;
                }
            }
            break;
        }
        Ok(top)
    }

    pub fn finalize(&mut self) -> Result<Option<Vec<(HashNumber, Justification)>>, Error> {
        let top = self.find_trunk_top()?;
        if &top == self.graph.get_root() {
            return Ok(None);
        }
        let (trunk, pruned) = self.graph.cut_trunk(top)?;
        self.compost_bin.extend(pruned);
        let minimal_number = self.minimal_number();
        self.compost_bin = self
            .compost_bin
            .drain()
            .filter(|x| x.number > minimal_number)
            .collect();
        Ok(Some(
            trunk
                .into_iter()
                .map(
                    |(hashnumber, vertex)| -> Result<(HashNumber, Justification), Error> {
                        Ok((hashnumber, vertex.justification.ok_or(Error::CriticalBug)?))
                    },
                )
                .collect::<Result<Vec<(HashNumber, Justification)>, Error>>()?,
        ))
    }

    // pub fn state_summary ...
}
