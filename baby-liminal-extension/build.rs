// This build script is only used for the runtime benchmarking setup. We don't need to do anything here if we are
// not building the runtime with the `runtime-benchmarks` feature.
#[cfg(not(feature = "runtime-benchmarks"))]
fn main() {}

#[cfg(feature = "runtime-benchmarks")]
use {
    artifacts::generate_artifacts,
    halo2_proofs::{halo2curves::bn256::Bn256, poly::kzg::commitment::ParamsKZG},
    std::{env, fs, path::Path},
};

#[cfg(feature = "runtime-benchmarks")]
fn main() {
    // We rerun the build script only if this file changes. SNARK artifacts generation doesn't
    // depend on any of the source files.
    println!("cargo:rerun-if-changed=build.rs");

    const CIRCUIT_MAX_K: u32 = 12;
    let params = ParamsKZG::<Bn256>::setup(CIRCUIT_MAX_K, ParamsKZG::<Bn256>::mock_rng());

    let artifacts = generate_artifacts::<5, 10>(&params);

    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("bench_artifact_5_10_vk");
    fs::write(&dest_path, artifacts.verification_key).unwrap();

    let dest_path = Path::new(&out_dir).join("bench_artifact_5_10_proof");
    fs::write(&dest_path, artifacts.proof).unwrap();

    let dest_path = Path::new(&out_dir).join("bench_artifact_5_10_input");
    fs::write(&dest_path, artifacts.public_input).unwrap();
}

#[cfg(feature = "runtime-benchmarks")]
mod artifacts {
    use halo2_proofs::{
        halo2curves::bn256::{Bn256, Fr, G1Affine},
        plonk::{create_proof, keygen_pk, keygen_vk, VerifyingKey},
        poly::{
            commitment::Params,
            kzg::{commitment::ParamsKZG, multiopen::ProverGWC},
        },
        transcript::{Blake2bWrite, Challenge255, TranscriptWriterBuffer},
    };

    use crate::circuit::BenchCircuit;

    pub struct Artifacts {
        /// The verification key.
        pub verification_key: Vec<u8>,
        /// The proof.
        pub proof: Vec<u8>,
        /// The public input.
        pub public_input: Vec<u8>,
    }

    pub fn generate_artifacts<const INSTANCES: usize, const ROW_BLOWUP: usize>(
        params: &ParamsKZG<Bn256>,
    ) -> Artifacts {
        let circuit = BenchCircuit::<INSTANCES, ROW_BLOWUP>::natural_numbers();
        let instances = (0..INSTANCES)
            .map(|i| Fr::from((i * i) as u64))
            .collect::<Vec<_>>();

        let vk = keygen_vk(params, &circuit).expect("vk should not fail");
        let pk = keygen_pk(params, vk.clone(), &circuit).expect("pk should not fail");

        let mut transcript = Blake2bWrite::<_, _, Challenge255<_>>::init(vec![]);
        create_proof::<_, ProverGWC<'_, Bn256>, _, _, _, _>(
            params,
            &pk,
            &[circuit],
            &[&[&instances]],
            ParamsKZG::<Bn256>::mock_rng(),
            &mut transcript,
        )
        .expect("prover should not fail");

        Artifacts {
            verification_key: serialize_vk(vk, params.k()),
            proof: transcript.finalize(),
            public_input: instances.iter().flat_map(|i| i.to_bytes()).collect(),
        }
    }

    fn serialize_vk(vk: VerifyingKey<G1Affine>, k: u32) -> Vec<u8> {
        let mut buffer = Vec::new();
        buffer.extend(k.to_le_bytes());
        buffer.extend(vk.to_bytes(halo2_proofs::SerdeFormat::RawBytesUnchecked));
        buffer
    }
}

#[cfg(feature = "runtime-benchmarks")]
mod circuit {
    use halo2_proofs::{
        circuit::{Layouter, Region, Value},
        halo2curves::bn256::Fr,
        plonk::{Circuit, ConstraintSystem, Error},
        standard_plonk::{StandardPlonk, StandardPlonkConfig},
    };

    pub struct BenchCircuit<const INSTANCES: usize, const ROW_BLOWUP: usize> {
        roots: [Fr; INSTANCES],
    }

    impl<const INSTANCES: usize, const ROW_BLOWUP: usize> Default
        for BenchCircuit<INSTANCES, ROW_BLOWUP>
    {
        fn default() -> Self {
            BenchCircuit {
                roots: [Fr::zero(); INSTANCES],
            }
        }
    }

    impl<const INSTANCES: usize, const ROW_BLOWUP: usize> BenchCircuit<INSTANCES, ROW_BLOWUP> {
        pub fn natural_numbers() -> Self {
            let roots = (0..INSTANCES)
                .map(|i| Fr::from(i as u64))
                .collect::<Vec<_>>();
            Self {
                roots: roots.try_into().unwrap(),
            }
        }

        fn neg_root_square(
            &self,
            idx: usize,
            region: &mut Region<Fr>,
            config: &StandardPlonkConfig<Fr>,
            offset: usize,
        ) -> Result<(), Error> {
            region.assign_advice(|| "", config.a, offset, || Value::known(self.roots[idx]))?;
            region.assign_advice(|| "", config.b, offset, || Value::known(self.roots[idx]))?;
            region.assign_fixed(|| "", config.q_ab, offset, || Value::known(-Fr::one()))?;
            Ok(())
        }
    }

    impl<const INSTANCES: usize, const ROW_BLOWUP: usize> Circuit<Fr>
        for BenchCircuit<INSTANCES, ROW_BLOWUP>
    {
        type Config = <StandardPlonk as Circuit<Fr>>::Config;
        type FloorPlanner = <StandardPlonk as Circuit<Fr>>::FloorPlanner;

        fn without_witnesses(&self) -> Self {
            BenchCircuit::default()
        }

        fn configure(meta: &mut ConstraintSystem<Fr>) -> Self::Config {
            StandardPlonk::configure(meta)
        }

        fn synthesize(
            &self,
            config: Self::Config,
            mut layouter: impl Layouter<Fr>,
        ) -> Result<(), Error> {
            for instance_idx in 0..INSTANCES {
                // For every instance, we ensure that the corresponding advice is indeed a square root of it.
                layouter.assign_region(
                    || "",
                    |mut region| {
                        self.neg_root_square(instance_idx, &mut region, &config, instance_idx)
                    },
                )?;

                // We also do some dummy work to blow up the number of rows.
                for row in 0..(ROW_BLOWUP - 1) {
                    let offset = INSTANCES + instance_idx * ROW_BLOWUP + row;
                    layouter.assign_region(
                        || "",
                        |mut region| {
                            self.neg_root_square(instance_idx, &mut region, &config, offset)?;

                            region.assign_advice_from_instance(
                                || "",
                                config.instance,
                                instance_idx,
                                config.c,
                                offset,
                            )?;
                            region.assign_fixed(
                                || "",
                                config.q_c,
                                offset,
                                || Value::known(Fr::one()),
                            )?;
                            Ok(())
                        },
                    )?;
                }
            }

            Ok(())
        }
    }
}
