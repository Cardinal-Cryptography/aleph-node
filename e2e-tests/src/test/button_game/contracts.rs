use aleph_client::{
    contract::{
        util::{to_account_id, to_bool, to_u128},
        ContractInstance,
    },
    AnyConnection, Balance, Connection, KeyPair, SignedConnection,
};
use anyhow::{Context, Result};
use sp_core::{
    crypto::{AccountId32, AccountId32 as AccountId, Ss58Codec},
    Pair,
};

use crate::Config;

pub trait AsContractInstance {
    fn as_contract(&self) -> &ContractInstance;
}

pub trait ToAccount {
    fn to_account(&self) -> AccountId32;
}

impl ToAccount for KeyPair {
    fn to_account(&self) -> AccountId32 {
        self.public().into()
    }
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

    pub fn is_dead<C: AnyConnection>(&self, conn: &C) -> Result<bool> {
        self.contract.contract_read0(conn, "is_dead").map(to_bool)?
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

    pub fn press(&self, conn: &SignedConnection) -> Result<()> {
        self.contract.contract_exec0(conn, "press")
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

impl ToAccount for ButtonInstance {
    fn to_account(&self) -> AccountId {
        self.as_contract().address().clone()
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
        to: &AccountId32,
        amount: Balance,
    ) -> Result<()> {
        self.contract.contract_exec(
            conn,
            "PSP22::transfer",
            &[to.to_string().as_str(), amount.to_string().as_str(), "0x00"],
        )
    }

    pub fn mint(&self, conn: &SignedConnection, to: &AccountId32, amount: Balance) -> Result<()> {
        self.contract.contract_exec(
            conn,
            "PSP22Mintable::mint",
            &[to.to_string().as_str(), amount.to_string().as_str()],
        )
    }

    pub fn approve(
        &self,
        conn: &SignedConnection,
        spender: &AccountId32,
        value: Balance,
    ) -> Result<()> {
        self.contract.contract_exec(
            conn,
            "PSP22::approve",
            &[spender.to_string().as_str(), value.to_string().as_str()],
        )
    }

    pub fn balance_of(&self, conn: &Connection, account: &AccountId32) -> Result<Balance> {
        to_u128(self.contract.contract_read(
            conn,
            "PSP22::balance_of",
            &[account.to_string().as_str()],
        )?)
    }
}

impl AsContractInstance for PSP22TokenInstance {
    fn as_contract(&self) -> &ContractInstance {
        &self.contract
    }
}

impl ToAccount for PSP22TokenInstance {
    fn to_account(&self) -> AccountId32 {
        self.contract.address().clone()
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

    pub fn reset(&self, conn: &SignedConnection) -> Result<()> {
        self.contract.contract_exec0(conn, "reset")
    }

    pub fn buy(&self, conn: &SignedConnection, max_price: Option<Balance>) -> Result<()> {
        let max_price = max_price.map_or_else(|| "None".to_string(), |x| format!("Some({})", x));

        self.contract
            .contract_exec(conn, "buy", &[max_price.as_str()])
    }

    pub fn price<C: AnyConnection>(&self, conn: &C) -> Result<Balance> {
        to_u128(self.contract.contract_read0(conn, "price")?)
    }
}

impl AsContractInstance for MarketplaceInstance {
    fn as_contract(&self) -> &ContractInstance {
        &self.contract
    }
}

impl ToAccount for MarketplaceInstance {
    fn to_account(&self) -> AccountId {
        self.contract.address().clone()
    }
}
