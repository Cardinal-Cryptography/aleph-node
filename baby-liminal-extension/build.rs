// This build script is only used for the runtime benchmarking setup. We don't need to do anything here if we are
// not building the runtime with the `runtime-benchmarks` feature.
#[cfg(not(feature = "runtime-benchmarks"))]
fn main() {}

#[cfg(feature = "runtime-benchmarks")]
fn main() {
    // We rerun the build script only if this file changes. SNARK artifacts generation doesn't
    // depend on any of the source files.
    println!("cargo:rerun-if-changed=build.rs");
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
        fn neg_root_square(
            &self,
            idx: usize,
            region: &mut Region<Fr>,
            config: &StandardPlonkConfig<Fr>,
            offset: usize,
        ) -> Result<(), Error> {
            region.assign_advice(|| "", config.a, offset, || Value::known(*self.roots[idx]))?;
            region.assign_advice(|| "", config.b, offset, || Value::known(*self.roots[idx]))?;
            region.assign_fixed(|| "", config.q_ab, offset, || Value::known(-Fr::one()))?;
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
