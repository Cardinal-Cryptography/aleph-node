use aleph_client::{
    contract::ContractInstance, AnyConnection, Balance, Connection, SignedConnection,
};
use anyhow::{Context, Result};
use sp_core::crypto::{AccountId32 as AccountId, Ss58Codec};

use crate::Config;

/// A wrapper around a button game contract.
///
/// The methods on this type match contract methods.
#[derive(Debug)]
pub(super) struct ButtonInstance {
    contract: ContractInstance,
}

impl ButtonInstance {
    pub fn new(config: &Config, button_address: &Option<String>) -> Result<Self> {
        let button_address = button_address
            .clone()
            .context("Button game address not set.")?;
        let button_address = AccountId::from_string(&button_address)?;
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
        self.contract.contract_read0(conn, "deadline")?.try_into()
    }

    pub fn is_dead<C: AnyConnection>(&self, conn: &C) -> Result<bool> {
        self.contract.contract_read0(conn, "is_dead")?.try_into()
    }

    pub fn ticket_token<C: AnyConnection>(&self, conn: &C) -> Result<AccountId> {
        self.contract
            .contract_read0(conn, "ticket_token")?
            .try_into()
    }

    pub fn reward_token<C: AnyConnection>(&self, conn: &C) -> Result<AccountId> {
        self.contract
            .contract_read0(conn, "reward_token")?
            .try_into()
    }

    pub fn marketplace<C: AnyConnection>(&self, conn: &C) -> Result<AccountId> {
        self.contract
            .contract_read0(conn, "marketplace")?
            .try_into()
    }

    pub fn press(&self, conn: &SignedConnection) -> Result<()> {
        self.contract.contract_exec0(conn, "press")
    }

    pub fn reset(&self, conn: &SignedConnection) -> Result<()> {
        self.contract.contract_exec0(conn, "reset")
    }
}

impl<'a> From<&'a ButtonInstance> for &'a ContractInstance {
    fn from(button: &'a ButtonInstance) -> Self {
        &button.contract
    }
}

impl From<&ButtonInstance> for AccountId {
    fn from(button: &ButtonInstance) -> Self {
        button.contract.address().clone()
    }
}

/// A wrapper around a PSP22 contract.
///
/// The methods on this type match contract methods.
#[derive(Debug)]
pub(super) struct PSP22TokenInstance {
    contract: ContractInstance,
}

impl PSP22TokenInstance {
    pub fn new(address: AccountId, metadata_path: &Option<String>) -> Result<Self> {
        let metadata_path = metadata_path
            .as_ref()
            .context("PSP22Token metadata not set.")?;
        Ok(Self {
            contract: ContractInstance::new(address, metadata_path)?,
        })
    }

    pub fn transfer(&self, conn: &SignedConnection, to: &AccountId, amount: Balance) -> Result<()> {
        self.contract.contract_exec(
            conn,
            "PSP22::transfer",
            &[to.to_string().as_str(), amount.to_string().as_str(), "0x00"],
        )
    }

    pub fn mint(&self, conn: &SignedConnection, to: &AccountId, amount: Balance) -> Result<()> {
        self.contract.contract_exec(
            conn,
            "PSP22Mintable::mint",
            &[to.to_string().as_str(), amount.to_string().as_str()],
        )
    }

    pub fn approve(
        &self,
        conn: &SignedConnection,
        spender: &AccountId,
        value: Balance,
    ) -> Result<()> {
        self.contract.contract_exec(
            conn,
            "PSP22::approve",
            &[spender.to_string().as_str(), value.to_string().as_str()],
        )
    }

    pub fn balance_of(&self, conn: &Connection, account: &AccountId) -> Result<Balance> {
        self.contract
            .contract_read(conn, "PSP22::balance_of", &[account.to_string().as_str()])?
            .try_into()
    }
}

impl<'a> From<&'a PSP22TokenInstance> for &'a ContractInstance {
    fn from(token: &'a PSP22TokenInstance) -> Self {
        &token.contract
    }
}

impl From<&PSP22TokenInstance> for AccountId {
    fn from(token: &PSP22TokenInstance) -> AccountId {
        token.contract.address().clone()
    }
}

/// A wrapper around a marketplace contract instance.
///
/// The methods on this type match contract methods.
#[derive(Debug)]
pub(super) struct MarketplaceInstance {
    contract: ContractInstance,
}

impl MarketplaceInstance {
    pub fn new(address: AccountId, metadata_path: &Option<String>) -> Result<Self> {
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

    pub fn set_code(
        &self,
        conn: &SignedConnection,
        code_hash: &String,
        migration_fn_selector: Option<String>,
    ) -> Result<()> {
        let selector_opt_str =
            migration_fn_selector.map_or_else(|| "None".to_string(), |x| format!("Some({})", x));
        self.contract
            .contract_exec(conn, "set_code", &[code_hash, selector_opt_str.as_str()])
    }

    pub fn migrate(&self, conn: &SignedConnection) -> Result<()> {
        self.contract.contract_exec0(conn, "migrate")
    }

    pub fn migration_performed(&self, conn: &SignedConnection) -> Result<bool> {
        self.contract
            .contract_read0(conn, "migration_performed")?
            .try_into()
    }

    pub fn buy(&self, conn: &SignedConnection, max_price: Option<Balance>) -> Result<()> {
        let max_price = max_price.map_or_else(|| "None".to_string(), |x| format!("Some({})", x));

        self.contract
            .contract_exec(conn, "buy", &[max_price.as_str()])
    }

    pub fn price<C: AnyConnection>(&self, conn: &C) -> Result<Balance> {
        self.contract.contract_read0(conn, "price")?.try_into()
    }

    // Access inner ContractInstance
    pub fn contract(&self) -> &ContractInstance {
        &self.contract
    }
}

impl<'a> From<&'a MarketplaceInstance> for &'a ContractInstance {
    fn from(marketplace: &'a MarketplaceInstance) -> Self {
        &marketplace.contract
    }
}

impl From<&MarketplaceInstance> for AccountId {
    fn from(marketplace: &MarketplaceInstance) -> AccountId {
        marketplace.contract.address().clone()
    }
}
