use std::fmt::Debug;

use ark_poly::univariate::DensePolynomial;
use ark_poly_commit::marlin_pc::MarlinKZG10;
use ark_relations::r1cs::ConstraintSynthesizer;
use ark_serialize::CanonicalDeserialize;
use blake2::Blake2s;
use traits::{NonUniversalSystem, ProvingSystem};

use crate::{environment::traits::UniversalSystem, serialization::serialize};

// For now, we can settle with these types.
/// Common pairing engine.
pub type PairingEngine = ark_bls12_381::Bls12_381;
/// Common scalar field.
pub type CircuitField = ark_bls12_381::Fr;

// Systems with hardcoded parameters.
type Groth16 = ark_groth16::Groth16<PairingEngine>;
type GM17 = ark_gm17::GM17<PairingEngine>;
type MarlinPolynomialCommitment = MarlinKZG10<PairingEngine, DensePolynomial<CircuitField>>;
type Marlin = ark_marlin::Marlin<CircuitField, MarlinPolynomialCommitment, Blake2s>;

/// All available non universal proving systems.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum NonUniversalProvingSystem {
    Groth16,
    Gm17,
}

/// All available universal proving systems.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum UniversalProvingSystem {
    Marlin,
}

/// Any proving system.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum SomeProvingSystem {
    NonUniversal(NonUniversalProvingSystem),
    Universal(UniversalProvingSystem),
}

/// Common API for all systems.
impl SomeProvingSystem {
    pub fn id(&self) -> String {
        match self {
            SomeProvingSystem::NonUniversal(s) => s.id(),
            SomeProvingSystem::Universal(s) => s.id(),
        }
    }

    /// Generates proof for `circuit` using proving key `pk`. Returns serialized proof.
    pub fn prove<C: ConstraintSynthesizer<CircuitField>>(
        &self,
        circuit: C,
        pk: Vec<u8>,
    ) -> Vec<u8> {
        match self {
            SomeProvingSystem::NonUniversal(NonUniversalProvingSystem::Groth16) => {
                self._prove::<_, Groth16>(circuit, pk)
            }
            SomeProvingSystem::NonUniversal(NonUniversalProvingSystem::Gm17) => {
                self._prove::<_, GM17>(circuit, pk)
            }
            SomeProvingSystem::Universal(UniversalProvingSystem::Marlin) => {
                self._prove::<_, Marlin>(circuit, pk)
            }
        }
    }

    fn _prove<C: ConstraintSynthesizer<CircuitField>, S: ProvingSystem>(
        &self,
        circuit: C,
        pk: Vec<u8>,
    ) -> Vec<u8> {
        let pk = <S::ProvingKey>::deserialize(&*pk).expect("Failed to deserialize proving key");
        let proof = S::prove(&pk, circuit);
        serialize(&proof)
    }
}

/// Serialized keys.
pub struct RawKeys {
    pub pk: Vec<u8>,
    pub vk: Vec<u8>,
}

/// API available only for non universal proving systems.
impl NonUniversalProvingSystem {
    pub fn id(&self) -> String {
        format!("{:?}", self).to_lowercase()
    }

    /// Generates proving and verifying key for `circuit`. Returns serialized keys.
    pub fn generate_keys<C: ConstraintSynthesizer<CircuitField>>(&self, circuit: C) -> RawKeys {
        match self {
            NonUniversalProvingSystem::Groth16 => self._generate_keys::<_, Groth16>(circuit),
            NonUniversalProvingSystem::Gm17 => self._generate_keys::<_, GM17>(circuit),
        }
    }

    fn _generate_keys<C: ConstraintSynthesizer<CircuitField>, S: NonUniversalSystem>(
        &self,
        circuit: C,
    ) -> RawKeys {
        let (pk, vk) = S::generate_keys(circuit);
        RawKeys {
            pk: serialize(&pk),
            vk: serialize(&vk),
        }
    }
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
                self._generate_srs::<Marlin>(num_constraints, num_variables, degree)
            }
        }
    }

    fn _generate_srs<S: UniversalSystem>(
        &self,
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
            UniversalProvingSystem::Marlin => self._generate_keys::<_, Marlin>(circuit, srs),
        }
    }

    fn _generate_keys<C: ConstraintSynthesizer<CircuitField>, S: UniversalSystem>(
        &self,
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

pub mod traits {
    use ark_relations::r1cs::ConstraintSynthesizer;
    use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};

    use super::CircuitField;

    /// Common API for every proving system.
    pub trait ProvingSystem {
        type Proof: CanonicalSerialize + CanonicalDeserialize;
        type ProvingKey: CanonicalSerialize + CanonicalDeserialize;
        type VerifyingKey: CanonicalSerialize + CanonicalDeserialize;

        /// Generates proof for `circuit` using proving key `pk`
        fn prove<C: ConstraintSynthesizer<CircuitField>>(
            pk: &Self::ProvingKey,
            circuit: C,
        ) -> Self::Proof;
    }

    /// Common API for every universal proving system.
    pub trait UniversalSystem: ProvingSystem {
        type Srs: CanonicalSerialize + CanonicalDeserialize;

        /// Generates SRS.
        fn generate_srs(num_constraints: usize, num_variables: usize, degree: usize) -> Self::Srs;

        /// Generates proving and verifying key for `circuit` using `srs`.
        fn generate_keys<C: ConstraintSynthesizer<CircuitField>>(
            circuit: C,
            srs: &Self::Srs,
        ) -> (Self::ProvingKey, Self::VerifyingKey);
    }

    /// Common API for every non universal proving system.
    pub trait NonUniversalSystem: ProvingSystem {
        /// Generates proving and verifying key for `circuit`.
        fn generate_keys<C: ConstraintSynthesizer<CircuitField>>(
            circuit: C,
        ) -> (Self::ProvingKey, Self::VerifyingKey);
    }
}

mod trait_implementations {
    use ark_relations::r1cs::ConstraintSynthesizer;
    use ark_snark::SNARK;
    use ark_std::rand::{rngs::StdRng, SeedableRng};

    use crate::environment::{
        traits::{NonUniversalSystem, ProvingSystem, UniversalSystem},
        CircuitField, Groth16, Marlin, MarlinPolynomialCommitment, GM17,
    };

    fn dummy_rng() -> StdRng {
        StdRng::from_seed([0u8; 32])
    }

    // Unfortunately, Groth16, GM17 and Marlin don't have any common supertrait, and therefore,
    // we cannot provide any blanket implementation without running into damned `upstream crates may
    // add a new impl of trait` error (see https://github.com/rust-lang/rfcs/issues/2758).
    // Tfu. Disgusting.

    /// This macro takes a type `system` as the only argument and provides `ProvingSystem` and
    /// `NonUniversalSystem` implementations for it.
    ///
    /// `system` should implement `SNARK<CircuitField>` trait.  
    macro_rules! impl_non_universal_system_for_snark {
        ($system:ty) => {
            impl ProvingSystem for $system {
                type Proof = <$system as SNARK<CircuitField>>::Proof;
                type ProvingKey = <$system as SNARK<CircuitField>>::ProvingKey;
                type VerifyingKey = <$system as SNARK<CircuitField>>::VerifyingKey;

                fn prove<C: ConstraintSynthesizer<CircuitField>>(
                    pk: &Self::ProvingKey,
                    circuit: C,
                ) -> Self::Proof {
                    let mut rng = dummy_rng();
                    <$system as SNARK<CircuitField>>::prove(pk, circuit, &mut rng)
                        .expect("Failed to generate keys")
                }
            }

            impl NonUniversalSystem for $system {
                fn generate_keys<C: ConstraintSynthesizer<CircuitField>>(
                    circuit: C,
                ) -> (Self::ProvingKey, Self::VerifyingKey) {
                    let mut rng = dummy_rng();
                    <$system as SNARK<CircuitField>>::circuit_specific_setup(circuit, &mut rng)
                        .expect("Failed to generate keys")
                }
            }
        };
    }

    impl_non_universal_system_for_snark!(Groth16);
    impl_non_universal_system_for_snark!(GM17);

    impl ProvingSystem for Marlin {
        type Proof = ark_marlin::Proof<CircuitField, MarlinPolynomialCommitment>;
        type ProvingKey = ark_marlin::IndexProverKey<CircuitField, MarlinPolynomialCommitment>;
        type VerifyingKey = ark_marlin::IndexVerifierKey<CircuitField, MarlinPolynomialCommitment>;

        fn prove<C: ConstraintSynthesizer<CircuitField>>(
            pk: &Self::ProvingKey,
            circuit: C,
        ) -> Self::Proof {
            let mut rng = dummy_rng();
            Marlin::prove(pk, circuit, &mut rng).expect("Failed to generate proof")
        }
    }

    impl UniversalSystem for Marlin {
        type Srs = ark_marlin::UniversalSRS<CircuitField, MarlinPolynomialCommitment>;

        fn generate_srs(num_constraints: usize, num_variables: usize, degree: usize) -> Self::Srs {
            let mut rng = dummy_rng();
            Marlin::universal_setup(num_constraints, num_variables, degree, &mut rng)
                .expect("Failed to generate SRS")
        }

        fn generate_keys<C: ConstraintSynthesizer<CircuitField>>(
            circuit: C,
            srs: &Self::Srs,
        ) -> (Self::ProvingKey, Self::VerifyingKey) {
            Marlin::index(srs, circuit).expect(
                "Failed to generate keys from SRS (it might be the case, that the circuit is \
                larger than the SRS allows).",
            )
        }
    }
}
