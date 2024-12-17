use crate::{data_io::AlephData, Hasher};

mod scorer;
mod service;

pub use service::Service;

type Batch<UH> = Vec<current_aleph_bft::OrderedUnit<AlephData<UH>, Hasher>>;
