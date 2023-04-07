use ark_ec::TEModelParameters;
use ark_ed_on_bls12_377::{EdwardsAffine, EdwardsParameters, Fq, Fr};
use ark_ff::PrimeField;
use ark_r1cs_std::groups::curves::twisted_edwards::AffineVar;
use ark_r1cs_std::ToBitsGadget;
use ark_r1cs_std::{alloc::AllocVar, eq::EqGadget};
use ark_relations::{
    ns,
    r1cs::{
        ConstraintSynthesizer, ConstraintSystemRef, SynthesisError,
        SynthesisError::AssignmentMissing,
    },
};
use ark_serialize::CanonicalSerialize;
use ark_std::{marker::PhantomData, vec, vec::Vec, UniformRand};

type FpVar = ark_r1cs_std::fields::fp::FpVar<Fr>;
type FqVar = ark_r1cs_std::fields::fp::FpVar<Fq>;
type AffVar = ark_r1cs_std::groups::curves::twisted_edwards::AffineVar<EdwardsParameters, FqVar>;
type CircuitField = Fr;
type Secret = Fr; // Scalar field

#[derive(Clone)]
pub struct PoE<S: State> {
    pub point_x: Option<Fq>,
    pub point_y: Option<Fq>,
    pub exp: Option<Secret>,
    _phantom: PhantomData<S>,
}

impl PoE<NoInput> {
    pub fn without_input() -> Self {
        PoE {
            point_x: None,
            point_y: None,
            exp: None,
            _phantom: PhantomData,
        }
    }
}

impl PoE<OnlyPublicInput> {
    pub fn with_public_input(point_x: Fq, point_y: Fq) -> Self {
        PoE {
            point_x: Some(point_x),
            point_y: Some(point_y),
            exp: None,
            _phantom: PhantomData,
        }
    }
}

impl PoE<FullInput> {
    pub fn with_full_input(point_x: Fq, point_y: Fq, exp: Secret) -> Self {
        PoE {
            point_x: Some(point_x),
            point_y: Some(point_y),
            exp: Some(exp),
            _phantom: PhantomData,
        }
    }
}

impl<S: State> ConstraintSynthesizer<CircuitField> for PoE<S> {
    fn generate_constraints(
        self,
        cs: ConstraintSystemRef<CircuitField>,
    ) -> Result<(), SynthesisError> {
        let generator = generator();
        let generator = AffVar::new_constant(ns!(cs, "generator"), generator);
        let generator = AffineVar::new_constant(ns!(cs, "generator"), || generator);

        // let expected_point = AffVar::new_input(ns!(cs, "point"), || {
        //     (
        //         self.point_x.ok_or(AssignmentMissing)?,
        //         self.point_y.ok_or(AssignmentMissing)?,
        //     )
        //         .into()
        // })?;

        // let exp = FpVar::new_witness(ns!(cs, "exp"), || self.exp.ok_or(AssignmentMissing))?;
        // let point = generator.scalar_mul_le(exp.to_bits_le());

        // expected_point.enforce_equal(point)?;

        Ok(())
    }
}

fn generator() -> EdwardsAffine {
    let (x, y) = <EdwardsParameters as TEModelParameters>::AFFINE_GENERATOR_COEFFS;
    EdwardsAffine::new(x, y)
}

// impl<S: WithPublicInput> GetPublicInput<CircuitField> for PoE<S> {
//     fn public_input(&self) -> Vec<CircuitField> {
//         vec![self.point_x.unwrap(), self.point_y.unwrap()]
//     }
// }

// mod tests {
//     use ark_bls12_381::Bls12_381;
//     use ark_groth16::Groth16;
//     use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystem, ConstraintSystemRef};
//     use ark_snark::SNARK;

//     use super::*;

//     #[test]
//     fn poe_constraints_correctness() {
//         let mut rng = ark_std::test_rng();
//         let exp = Secret::rand(&mut rng);
//         let generator = generator();
//         let point = generator * exp;

//         let circuit = PoE::with_full_input(point.x, point.y, exp);

//         let cs: ConstraintSystemRef<Fr> = ConstraintSystem::new_ref();
//         circuit.generate_constraints(cs.clone()).unwrap();

//         let is_satisfied = cs.is_satisfied().unwrap();
//         if !is_satisfied {
//             println!("{:?}", cs.which_is_unsatisfied());
//         }

//         assert!(is_satisfied);
//     }

//     #[test]
//     fn poe_proving_procedure() {
//         let circuit_wo_input = PoE::without_input();

//         let mut rng = ark_std::test_rng();
//         let (pk, vk) =
//             Groth16::<Bls12_381>::circuit_specific_setup(circuit_wo_input, &mut rng).unwrap();

//         let exp = Secret::rand(&mut rng);
//         let generator = generator();
//         let point = generator * exp;

//         let circuit_with_public_input = PoE::with_public_input(point.x, point.y);
//         let input = circuit_with_public_input.serialize_public_input();

//         let circuit_with_full_input = PoE::with_full_input(point, exp);

//         let proof = Groth16::prove(&pk, circuit_with_full_input, &mut rng).unwrap();
//         let valid_proof = Groth16::verify(&vk, &input, &proof).unwrap();
//         assert!(valid_proof);
//     }
// }

pub trait GetPublicInput<Field: PrimeField + CanonicalSerialize> {
    fn public_input(&self) -> Vec<Field> {
        vec![]
    }
}

#[derive(Clone, Debug)]
pub enum NoInput {}
#[derive(Clone, Debug)]
pub enum OnlyPublicInput {}
#[derive(Clone, Debug)]
pub enum FullInput {}

pub trait State {}
impl State for NoInput {}
impl State for OnlyPublicInput {}
impl State for FullInput {}

pub trait WithPublicInput: State {}
impl WithPublicInput for OnlyPublicInput {}
impl WithPublicInput for FullInput {}
