use clap::ValueEnum;
use relations::{
    serialize, CanonicalDeserialize, CircuitField, ConstraintSynthesizer, Marlin, RawKeys,
    UniversalSystem,
};

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
