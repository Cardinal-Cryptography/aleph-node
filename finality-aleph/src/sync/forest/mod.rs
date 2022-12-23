use std::collections::{HashMap, HashSet};

pub mod graph;
use graph::{Error, Forest as Graph};

#[derive(Clone, std::cmp::PartialEq, std::cmp::Eq, std::hash::Hash)]
pub struct Hash;
#[derive(Clone, std::cmp::PartialOrd, std::cmp::PartialEq, std::cmp::Eq, std::hash::Hash)]
pub struct Number;
#[derive(Clone, std::cmp::PartialEq, std::cmp::Eq, std::hash::Hash)]
pub struct HashNumber {
    hash: Hash,
    number: Number,
}
pub struct Header;
#[derive(std::cmp::PartialEq, std::cmp::Eq, std::hash::Hash)]
pub struct PeerID;

impl Header {
    fn parent(&self) -> HashNumber {
        HashNumber {
            hash: Hash,
            number: Number,
        }
    }
}

// struct Body;
pub struct Justification;

enum ForestJustification {
    Justification(Justification),
    Empty,
}

struct Vertex {
    header: Option<Header>,
    justification: Option<Justification>,
    important: bool,
    know_most: HashSet<PeerID>,
}

pub enum Request {
    Header(HashNumber),
    Block(HashNumber),
    JustificationsBelow(HashNumber),
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

    fn minimal_number(&self) -> &Number {
        &self.graph.get_root().number
    }

    fn filter_compost_bin(&mut self) {
        let minimal = self.minimal_number().clone();
        self.compost_bin = self
            .compost_bin
            .drain()
            .filter(|x| x.number > minimal)
            .collect();
    }

    fn is_relevant(&self, hashnumber: &HashNumber) -> bool {
        &hashnumber.number > self.minimal_number() && !self.compost_bin.contains(hashnumber)
    }

    fn is_important(&self, hashnumber: &HashNumber) -> Option<bool> {
        self.graph.get(hashnumber).map(|x| x.important)
    }

    // We want to prune a specific vertex whenever we know that it is irrelevant
    // for extending the blockchain. When we do this, we remove it from the forest
    // and put its hash+number into the compost bin if it is above the highest
    // justified block. Then we prune all its descendants.
    // This action never returns any requests.
    pub fn prune(&mut self, vertex: HashNumber) {}

    pub fn insert_hashnumber(
        &mut self,
        hashnumber: HashNumber,
        sender: Option<PeerID>,
        important: bool,
    ) -> Result<Option<Request>, Error> {
        #[derive(std::cmp::PartialEq)]
        enum PreviousState {
            Nonexistent,
            Unimportant,
            Important,
        }
        let previous_state: PreviousState;
        if !self.is_relevant(&hashnumber) {
            return Ok(None);
        }
        match self.graph.get_mut(&hashnumber) {
            Some(mut vertex) => {
                if vertex.justification.is_some() {
                    return Ok(None);
                }
                previous_state = if vertex.important {
                    PreviousState::Important
                } else {
                    PreviousState::Unimportant
                };
                if let Some(sender) = sender {
                    vertex.know_most.insert(sender);
                }
                if important {
                    vertex.important = true;
                    let mut parent = hashnumber.clone();
                    loop {
                        parent = match self.graph.get_parent_key(&parent) {
                            Some(k) => k.clone(),
                            None => break,
                        };
                        vertex = match self.graph.get_mut(&parent) {
                            Some(v) => v,
                            None => break,
                        };
                        vertex.important = true;
                    }
                }
            }
            None => {
                previous_state = PreviousState::Nonexistent;
                let mut know_most = HashSet::new();
                if let Some(sender) = sender {
                    know_most.insert(sender);
                }
                let vertex = Vertex {
                    header: None,
                    justification: None,
                    important,
                    know_most,
                };
                self.graph.insert(hashnumber.clone(), vertex, None)?;
            }
        }

        if important && previous_state != PreviousState::Important {
            return Ok(Some(Request::Block(hashnumber)));
        }
        if !important && previous_state == PreviousState::Nonexistent {
            return Ok(Some(Request::Header(hashnumber)));
        }
        Ok(None)
    }
}
