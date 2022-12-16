use std::str::FromStr;

use aleph_client::{contract::ContractInstance, AccountId, Connection, SignedConnection};
use anyhow::{Context, Result};

use crate::{
    config::setup_test,
    test::helpers::{basic_test_context, sign},
};

/// This test exercises the aleph-client code for interacting with contracts by testing a simple contract that maintains
/// some state and publishes some events.
#[tokio::test]
pub async fn adder() -> Result<()> {
    let config = setup_test();

    let (conn, _authority, account) = basic_test_context(config).await?;
    let contract = AdderInstance::new(
        &config.test_case_params.adder,
        &config.test_case_params.adder_metadata,
    )?;

    let before = contract.get(&conn).await?;
    contract.add(&sign(&conn, &account), 10).await?;
    let after = contract.get(&conn).await?;

    assert!(after == before + 10);

    Ok(())
}

pub(super) struct AdderInstance {
    contract: ContractInstance,
}

impl<'a> From<&'a AdderInstance> for &'a ContractInstance {
    fn from(instance: &'a AdderInstance) -> Self {
        &instance.contract
    }
}

impl<'a> From<&'a AdderInstance> for AccountId {
    fn from(instance: &'a AdderInstance) -> Self {
        instance.contract.address().clone()
    }
}

impl AdderInstance {
    pub fn new(address: &Option<String>, metadata_path: &Option<String>) -> Result<Self> {
        let address = address.as_ref().context("Adder contract address not set")?;
        let metadata_path = metadata_path
            .as_ref()
            .context("Adder contract metadata not set")?;

        let address = AccountId::from_str(address)
            .ok()
            .with_context(|| format!("Failed to parse address: {}", address))?;
        let contract = ContractInstance::new(address, metadata_path)?;
        Ok(Self { contract })
    }

    pub async fn get(&self, conn: &Connection) -> Result<u32> {
        self.contract.contract_read0(conn, "get").await
    }

    pub async fn add(&self, conn: &SignedConnection, value: u32) -> Result<()> {
        self.contract
            .contract_exec(conn, "add", &[value.to_string()])
            .await
    }
}
