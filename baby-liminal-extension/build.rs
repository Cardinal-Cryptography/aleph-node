use halo2_proofs::{
    circuit::Layouter,
    halo2curves::bn256::Fr,
    plonk::{Circuit, ConstraintSystem, Error},
    standard_plonk::StandardPlonk,
};

fn main() {
    // This build script is only used for the runtime benchmarking setup. We don't need to do anything here if we are
    // not building the runtime with the `runtime-benchmarks` feature.
    #[cfg(not(feature = "runtime-benchmarks"))]
    return;

    // We rerun the build script only if this file changes. SNARK artifacts generation doesn't
    // depend on any of the source files.
    println!("cargo:rerun-if-changed=build.rs");
}

struct BenchCircuit<const INSTANCES: usize, const ROW_BLOWUP: u32> {
    roots: [Fr; INSTANCES],
}

impl<const INSTANCES: usize, const ROW_BLOWUP: u32> Default
    for BenchCircuit<INSTANCES, ROW_BLOWUP>
{
    fn default() -> Self {
        BenchCircuit {
            roots: [Fr::zero(); INSTANCES],
        }
    }
}

impl<const INSTANCES: usize, const ROW_BLOWUP: u32> Circuit<Fr>
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
        _config: Self::Config,
        mut _layouter: impl Layouter<Fr>,
    ) -> Result<(), Error> {
        // layouter.assign_region(
        //     || "",
        //     |mut region| {
        //         region.assign_advice(|| "", config.a, 0, || Value::known(self.a))?;
        //         region.assign_fixed(|| "", config.q_a, 0, || Value::known(-Fr::one()))?;
        //         region.assign_advice(|| "", config.b, 0, || Value::known(self.b))?;
        //         region.assign_fixed(|| "", config.q_b, 0, || Value::known(-Fr::one()))?;
        //         Ok(())
        //     },
        // )
        Ok(())
    }
}
