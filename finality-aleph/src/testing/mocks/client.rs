use std::sync::Arc;

use substrate_test_client::{client, sc_client_db, sc_executor};
use substrate_test_runtime_client::{GenesisParameters, LocalExecutorDispatch};

use crate::block::mock::MockBlock;

// /// A `TestClient` with `test-runtime` builder.
pub type TestClientBuilder<E, B> =
    substrate_test_client::TestClientBuilder<MockBlock, E, B, GenesisParameters>;

/// Call executor for `kitchensink-runtime` `TestClient`.
pub type ExecutorDispatch = sc_executor::NativeElseWasmExecutor<LocalExecutorDispatch>;

/// Default backend type.
pub type Backend = sc_client_db::Backend<MockBlock>;

/// Test client type.
pub type TestClient = client::Client<
    Backend,
    client::LocalCallExecutor<MockBlock, Backend, ExecutorDispatch>,
    MockBlock,
    aleph_runtime::RuntimeApi,
>;

/// A `test-runtime` extensions to `TestClientBuilder`.
pub trait TestClientBuilderExt: Sized {
    /// Create test client builder.
    fn new() -> Self;

    /// Build the test client.
    fn build(self) -> TestClient;

    /// Build the test client and longest chain selector.
    fn build_with_longest_chain(
        self,
    ) -> (TestClient, sc_consensus::LongestChain<Backend, MockBlock>);

    /// Build the test client and the backend.
    fn build_with_backend(self) -> (TestClient, Arc<Backend>);
}

impl TestClientBuilderExt
    for substrate_test_client::TestClientBuilder<
        MockBlock,
        client::LocalCallExecutor<MockBlock, Backend, ExecutorDispatch>,
        Backend,
        GenesisParameters,
    >
{
    fn new() -> Self {
        Self::default()
    }

    fn build(self) -> TestClient {
        self.build_with_native_executor(None).0
    }

    fn build_with_longest_chain(
        self,
    ) -> (TestClient, sc_consensus::LongestChain<Backend, MockBlock>) {
        self.build_with_native_executor(None)
    }

    fn build_with_backend(self) -> (TestClient, Arc<Backend>) {
        let backend = self.backend();
        (self.build_with_native_executor(None).0, backend)
    }
}
