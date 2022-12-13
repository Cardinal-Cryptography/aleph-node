pub use systems::{NonUniversalProvingSystem, UniversalProvingSystem};

use crate::snark_relations::io::save_srs;

mod io;
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
