use halo2_proofs::{
    circuit::{Layouter, Value},
    plonk::{create_proof, keygen_pk, keygen_vk, Circuit, ConstraintSystem, Error},
    poly::kzg::{commitment::ParamsKZG, multiopen::ProverGWC},
    standard_plonk::StandardPlonk,
    transcript::{Blake2bWrite, Challenge255, TranscriptWriterBuffer},
    SerdeFormat,
};

use crate::snark_verifier::{
    implementation::{Curve, Fr},
    verify, CIRCUIT_MAX_K,
};

#[derive(Default)]
struct APlusBIsC {
    a: Fr,
    b: Fr,
}

impl Circuit<Fr> for APlusBIsC {
    type Config = <StandardPlonk as Circuit<Fr>>::Config;
    type FloorPlanner = <StandardPlonk as Circuit<Fr>>::FloorPlanner;

    fn without_witnesses(&self) -> Self {
        APlusBIsC::default()
    }

    fn configure(meta: &mut ConstraintSystem<Fr>) -> Self::Config {
        StandardPlonk::configure(meta)
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<Fr>,
    ) -> Result<(), Error> {
        layouter.assign_region(
            || "",
            |mut region| {
                region.assign_advice(|| "", config.a, 0, || Value::known(self.a))?;
                region.assign_fixed(|| "", config.q_a, 0, || Value::known(-Fr::one()))?;
                region.assign_advice(|| "", config.b, 0, || Value::known(self.b))?;
                region.assign_fixed(|| "", config.q_b, 0, || Value::known(-Fr::one()))?;
                Ok(())
            },
        )
    }
}

#[test]
fn accepts_correct_proof() {
    let circuit = APlusBIsC {
        a: Fr::from(1u64),
        b: Fr::from(2u64),
    };
    let instances = vec![Fr::from(3u64)];

    let params = ParamsKZG::<Curve>::setup(CIRCUIT_MAX_K, ParamsKZG::<Curve>::mock_rng());
    let vk = keygen_vk(&params, &circuit).expect("vk should not fail");
    let pk = keygen_pk(&params, vk.clone(), &circuit).expect("pk should not fail");

    let mut transcript = Blake2bWrite::<_, _, Challenge255<_>>::init(vec![]);
    create_proof::<_, ProverGWC<'_, Curve>, _, _, _, _>(
        &params,
        &pk,
        &[circuit],
        &[&[&instances]],
        rand::rngs::OsRng,
        &mut transcript,
    )
    .expect("prover should not fail");
    let proof = transcript.finalize();

    let x = verify(
        &proof,
        &instances
            .iter()
            .flat_map(|i| i.to_bytes())
            .collect::<Vec<_>>(),
        &vk.to_bytes(SerdeFormat::RawBytesUnchecked),
    );

    assert!(x.is_ok());
}
