use crate::{data_io::AlephData, Hasher};

mod scorer;
mod service;

pub use service::{Service, ServiceIO};

type Batch<UH> = Vec<legacy_aleph_bft::OrderedUnit<AlephData<UH>, Hasher>>;
