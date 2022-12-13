use clap::ValueEnum;
use relations::{
    serialize, CanonicalDeserialize, CircuitField, ConstraintSynthesizer, Marlin, RawKeys,
    UniversalSystem,
};

use crate::snark_relations::io::save_srs;

/// All available non universal proving systems.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, ValueEnum)]
pub enum NonUniversalProvingSystem {
    Groth16,
    Gm17,
}

/// All available universal proving systems.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, ValueEnum)]
pub enum UniversalProvingSystem {
    Marlin,
}

/// API available only for universal proving systems.
impl UniversalProvingSystem {
    pub fn id(&self) -> String {
        format!("{:?}", self).to_lowercase()
    }

    /// Generates SRS. Returns in serialized version.
    pub fn generate_srs(
        &self,
        num_constraints: usize,
        num_variables: usize,
        degree: usize,
    ) -> Vec<u8> {
        match self {
            UniversalProvingSystem::Marlin => {
                Self::_generate_srs::<Marlin>(num_constraints, num_variables, degree)
            }
        }
    }

    fn _generate_srs<S: UniversalSystem>(
        num_constraints: usize,
        num_variables: usize,
        degree: usize,
    ) -> Vec<u8> {
        let srs = S::generate_srs(num_constraints, num_variables, degree);
        serialize(&srs)
    }

    /// Generates proving and verifying key for `circuit` using `srs`. Returns serialized keys.
    pub fn generate_keys<C: ConstraintSynthesizer<CircuitField>>(
        &self,
        circuit: C,
        srs: Vec<u8>,
    ) -> RawKeys {
        match self {
            UniversalProvingSystem::Marlin => Self::_generate_keys::<_, Marlin>(circuit, srs),
        }
    }

    fn _generate_keys<C: ConstraintSynthesizer<CircuitField>, S: UniversalSystem>(
        circuit: C,
        srs: Vec<u8>,
    ) -> RawKeys {
        let srs =
            <<S as UniversalSystem>::Srs>::deserialize(&*srs).expect("Failed to deserialize srs");
        let (pk, vk) = S::generate_keys(circuit, &srs);
        RawKeys {
            pk: serialize(&pk),
            vk: serialize(&vk),
        }
    }
}

pub fn generate_srs(
    system: UniversalProvingSystem,
    num_constraints: usize,
    num_variables: usize,
    degree: usize,
) {
    let srs = system.generate_srs(num_constraints, num_variables, degree);
    save_srs(&srs, &system.id());
}

mod io {
    use std::{fs, path::PathBuf};

    fn save_bytes(bytes: &[u8], prefix: &str, identifier: &str) {
        let path = format!("{}.{}.bytes", prefix, identifier);
        fs::write(path, bytes).unwrap_or_else(|_| panic!("Failed to save {}", identifier));
    }

    pub fn save_srs(srs: &[u8], env_id: &str) {
        save_bytes(srs, env_id, "srs");
    }

    pub fn save_keys(rel_name: &str, env_id: &str, pk: &[u8], vk: &[u8]) {
        let prefix = format!("{}.{}", rel_name, env_id);
        save_bytes(pk, &prefix, "pk");
        save_bytes(vk, &prefix, "vk");
    }

    pub fn save_proving_artifacts(rel_name: &str, env_id: &str, proof: &[u8], input: &[u8]) {
        let prefix = format!("{}.{}", rel_name, env_id);
        save_bytes(proof, &prefix, "proof");
        save_bytes(input, &prefix, "public_input");
    }

    pub fn read_srs(srs_file: PathBuf) -> Vec<u8> {
        fs::read(srs_file).expect("Failed to read SRS from the provided path")
    }

    pub fn read_proving_key(proving_key_file: PathBuf) -> Vec<u8> {
        fs::read(proving_key_file).expect("Failed to read proving key from the provided path")
    }
}
