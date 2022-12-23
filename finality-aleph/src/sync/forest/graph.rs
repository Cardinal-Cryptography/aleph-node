use std::{
    collections::{HashMap, HashSet},
    iter::Iterator,
};

pub trait Key: Clone + std::cmp::Eq + std::hash::Hash {}
impl<T: Clone + std::cmp::Eq + std::hash::Hash> Key for T {}

struct Vertex<K: Key, V> {
    value: V,
    parent: Option<K>,
    children: HashSet<K>,
}

pub enum Error {
    KeyAlreadyExists,
    MissingKey,
    MissingChildKey,
    MissingParentKey,
    ParentAlreadySet,
    CriticalBug,
}

pub struct Forest<K: Key, V> {
    vertices: HashMap<K, Vertex<K, V>>,
    root: K,
    root_children: HashSet<K>,
}

impl<K: Key, V> Forest<K, V> {
    pub fn new(root: K) -> Self {
        Self {
            vertices: HashMap::new(),
            root,
            root_children: HashSet::new(),
        }
    }

    pub fn contains_key(&self, key: &K) -> bool {
        self.vertices.contains_key(key) || &self.root == key
    }

    pub fn insert(&mut self, key: K, value: V, parent: Option<K>) -> Result<(), Error> {
        if self.contains_key(&key) {
            return Err(Error::KeyAlreadyExists);
        }
        if let Some(parent) = parent.clone() {
            if !self.contains_key(&parent) {
                return Err(Error::MissingParentKey);
            }
            if self.root == parent {
                self.root_children.insert(key.clone());
            } else {
                self.vertices
                    .get_mut(&parent)
                    .ok_or(Error::CriticalBug)?
                    .children
                    .insert(key.clone());
            }
        }
        self.vertices.insert(
            key,
            Vertex {
                value,
                parent,
                children: HashSet::new(),
            },
        );
        Ok(())
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        self.vertices.get(key).map(|x| &x.value)
    }

    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        self.vertices.get_mut(key).map(|x| &mut x.value)
    }

    pub fn get_root(&self) -> &K {
        &self.root
    }

    pub fn get_parent_key(&mut self, key: &K) -> Option<&K> {
        self.vertices.get(key).map_or(None, |x| x.parent.as_ref())
    }

    pub fn set_parent(&mut self, child: K, parent: K) -> Result<(), Error> {
        // child must not be the root
        if !self.vertices.contains_key(&child) {
            return Err(Error::MissingChildKey);
        }
        if !self.contains_key(&parent) {
            return Err(Error::MissingParentKey);
        }
        let mut v_child = self.vertices.get_mut(&child).ok_or(Error::CriticalBug)?;
        if v_child.parent.is_some() {
            return Err(Error::ParentAlreadySet);
        }
        v_child.parent = Some(parent.clone());

        let children = if self.root == parent {
            &mut self.root_children
        } else {
            &mut self
                .vertices
                .get_mut(&parent)
                .ok_or(Error::CriticalBug)?
                .children
        };
        if children.contains(&child) {
            return Err(Error::CriticalBug);
        }
        children.insert(child);

        Ok(())
    }

    pub fn prune(&mut self, key: K) -> Result<Vec<(K, V)>, Error> {
        // cannot prune the root
        if !self.vertices.contains_key(&key) {
            return Err(Error::MissingKey);
        }
        Ok(vec![])
    }
}
