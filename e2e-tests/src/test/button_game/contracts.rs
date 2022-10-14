use aleph_client::{
    contract::{
        util::{to_account_id, to_u128},
        ContractInstance,
    },
    AnyConnection, Balance, Connection, SignedConnection,
};
use anyhow::{Context, Result};
use sp_core::crypto::{AccountId32, AccountId32 as AccountId, Ss58Codec};

use crate::Config;

pub trait AsContractInstance {
    fn as_contract(&self) -> &ContractInstance;
}

#[derive(Debug)]
pub(super) struct ButtonInstance {
    contract: ContractInstance,
}

impl ButtonInstance {
    pub fn new(config: &Config) -> Result<Self> {
        let button_address = config
            .test_case_params
            .button_game_contract
            .clone()
            .context("Button game address not set.")?;
        let button_address = AccountId32::from_string(&button_address)?;
        let metadata_path = config
            .test_case_params
            .button_game_metadata
            .clone()
            .context("Button game metadata path not set.")?;
        Ok(Self {
            contract: ContractInstance::new(button_address, &metadata_path)?,
        })
    }

    pub fn deadline<C: AnyConnection>(&self, conn: &C) -> Result<u128> {
        self.contract
            .contract_read0(conn, "deadline")
            .map(to_u128)?
    }

    pub fn ticket_token<C: AnyConnection>(&self, conn: &C) -> Result<AccountId> {
        self.contract
            .contract_read0(conn, "ticket_token")
            .map(to_account_id)?
    }

    pub fn reward_token<C: AnyConnection>(&self, conn: &C) -> Result<AccountId> {
        self.contract
            .contract_read0(conn, "reward_token")
            .map(to_account_id)?
    }

    pub fn marketplace<C: AnyConnection>(&self, conn: &C) -> Result<AccountId> {
        self.contract
            .contract_read0(conn, "marketplace")
            .map(to_account_id)?
    }

    pub fn reset(&self, conn: &SignedConnection) -> Result<()> {
        self.contract.contract_exec0(conn, "reset")
    }
}

impl AsContractInstance for ButtonInstance {
    fn as_contract(&self) -> &ContractInstance {
        &self.contract
    }
}

#[derive(Debug)]
pub(super) struct PSP22TokenInstance {
    contract: ContractInstance,
}

impl PSP22TokenInstance {
    pub fn new(address: AccountId32, metadata_path: &Option<String>) -> Result<Self> {
        let metadata_path = metadata_path
            .as_ref()
            .context("PSP22Token metadata not set.")?;
        Ok(Self {
            contract: ContractInstance::new(address, metadata_path)?,
        })
    }

    pub fn transfer(
        &self,
        conn: &SignedConnection,
        to: AccountId32,
        amount: Balance,
    ) -> Result<()> {
        self.contract.contract_exec(
            conn,
            "PSP22::transfer",
            vec![to.to_string().as_str(), amount.to_string().as_str(), "0x00"].as_slice(),
        )
    }

    pub fn balance_of(&self, conn: &Connection, account: AccountId32) -> Result<Balance> {
        to_u128(self.contract.contract_read(
            conn,
            "PSP22::balance_of",
            &vec![account.to_string().as_str()],
        )?)
    }
}

impl AsContractInstance for PSP22TokenInstance {
    fn as_contract(&self) -> &ContractInstance {
        &self.contract
    }
}

#[derive(Debug)]
pub(super) struct MarketplaceInstance {
    contract: ContractInstance,
}

impl MarketplaceInstance {
    pub fn new(address: AccountId32, metadata_path: &Option<String>) -> Result<Self> {
        Ok(Self {
            contract: ContractInstance::new(
                address,
                metadata_path
                    .as_ref()
                    .context("Marketplace metadata not set.")?,
            )?,
        })
    }
}

impl AsContractInstance for MarketplaceInstance {
    fn as_contract(&self) -> &ContractInstance {
        &self.contract
    }
}
