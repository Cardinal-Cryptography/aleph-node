use ark_bls12_377::Fr;
use ark_ec::TEModelParameters;
use ark_ed_on_bls12_377::{EdwardsAffine, EdwardsParameters, Fq as FqEd, Fr as FrEd};
use ark_ff::{BigInteger, PrimeField};
use ark_r1cs_std::{alloc::AllocVar, boolean::Boolean, eq::EqGadget, groups::CurveVar};
use ark_relations::{
    ns,
    r1cs::{ConstraintSynthesizer, ConstraintSystemRef, SynthesisError},
};
use ark_std::{vec, vec::Vec};

type FqEdVar = ark_r1cs_std::fields::fp::FpVar<FqEd>;
type AffVar = ark_r1cs_std::groups::curves::twisted_edwards::AffineVar<EdwardsParameters, FqEdVar>;
type CircuitField = Fr; // Scalar field
type Secret = FrEd; // Ed Scalar field

#[derive(Clone)]
pub struct PoE {
    pub point_x: FqEd,
    pub point_y: FqEd,
    pub exp: Secret,
}

impl PoE {
    pub fn new(point_x: FqEd, point_y: FqEd, exp: Secret) -> Self {
        Self {
            point_x,
            point_y,
            exp,
        }
    }

    pub fn public_input(&self) -> Vec<CircuitField> {
        vec![self.point_x, self.point_y]
    }
}

impl ConstraintSynthesizer<CircuitField> for PoE {
    fn generate_constraints(
        self,
        cs: ConstraintSystemRef<CircuitField>,
    ) -> Result<(), SynthesisError> {
        let generator = AffVar::new_constant(ns!(cs, "generator"), generator())?;

        let x = FqEdVar::new_input(ns!(cs, "point_x"), || Ok(self.point_x))?;
        let y = FqEdVar::new_input(ns!(cs, "point_y"), || Ok(self.point_y))?;

        let expected_point = AffVar::new(x, y);

        let bits = self.exp.into_repr().to_bits_le();

        let exp = Vec::<Boolean<_>>::new_witness(ns!(cs, "exp"), || Ok(bits))?;
        let point = generator.scalar_mul_le(exp.iter())?;

        expected_point.enforce_equal(&point)?;

        Ok(())
    }
}

pub fn generator() -> EdwardsAffine {
    let (x, y) = <EdwardsParameters as TEModelParameters>::AFFINE_GENERATOR_COEFFS;
    EdwardsAffine::new(x, y)
}

#[cfg(test)]
mod tests {
    use std::ops::MulAssign;

    use ark_bls12_377::Bls12_377;
    use ark_ff::UniformRand;
    use ark_groth16::Groth16;
    use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystem, ConstraintSystemRef};
    use ark_snark::SNARK;

    use super::*;

    #[test]
    fn poe_constraints_correctness() {
        let mut rng = ark_std::test_rng();
        let exp = Secret::rand(&mut rng);
        let mut generator = generator();

        generator.mul_assign(exp);
        let point = generator;

        let circuit = PoE::new(point.x, point.y, exp);

        let cs: ConstraintSystemRef<Fr> = ConstraintSystem::new_ref();
        circuit.generate_constraints(cs.clone()).unwrap();

        let is_satisfied = cs.is_satisfied().unwrap();
        if !is_satisfied {
            println!("{:?}", cs.which_is_unsatisfied());
        }

        assert!(is_satisfied);
    }

    #[test]
    fn poe_proving_procedure() {
        let mut rng = ark_std::test_rng();
        let exp = Secret::rand(&mut rng);
        let mut generator = generator();

        generator.mul_assign(exp);
        let point = generator;

        let circuit = PoE::new(point.x, point.y, exp);

        let (pk, vk) =
            Groth16::<Bls12_377>::circuit_specific_setup(circuit.clone(), &mut rng).unwrap();

        let input = circuit.public_input();

        let proof = Groth16::prove(&pk, circuit, &mut rng).unwrap();
        let valid_proof = Groth16::verify(&vk, &input, &proof).unwrap();
        assert!(valid_proof);
    }
}
