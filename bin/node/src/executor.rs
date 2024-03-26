use sc_executor::WasmExecutor;

#[cfg(feature = "runtime-benchmarks")]
type ExtendHostFunctions = (
    sp_io::SubstrateHostFunctions,
    aleph_runtime_interfaces::snark_verifier::HostFunctions,
    frame_benchmarking::benchmarking::HostFunctions,
);

#[cfg(not(feature = "runtime-benchmarks"))]
type ExtendHostFunctions = (
    sp_io::SubstrateHostFunctions,
    aleph_runtime_interfaces::snark_verifier::HostFunctions,
);

pub type AlephExecutor = WasmExecutor<ExtendHostFunctions>;
