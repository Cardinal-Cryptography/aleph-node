use std::path::PathBuf;

pub use systems::{NonUniversalProvingSystem, UniversalProvingSystem};

pub use self::relations::RelationArgs;
use crate::snark_relations::io::{read_srs, save_keys, save_srs};

mod io;
mod relations;
mod systems;

pub fn generate_srs(
    system: UniversalProvingSystem,
    num_constraints: usize,
    num_variables: usize,
    degree: usize,
) {
    let srs = system.generate_srs(num_constraints, num_variables, degree);
    save_srs(&srs, &system.id());
}

pub fn generate_keys_from_srs(
    relation: RelationArgs,
    system: UniversalProvingSystem,
    srs_file: PathBuf,
) {
    let srs = read_srs(srs_file);
    let keys = system.generate_keys(relation.clone(), srs);
    save_keys(&relation.id(), &system.id(), &keys.pk, &keys.vk);
}
