use std::{collections::HashSet, path::PathBuf, str::FromStr};

use aleph_primitives::{
    staking::{MIN_NOMINATOR_BOND, MIN_VALIDATOR_BOND},
    AuthorityId as AlephId, ADDRESSES_ENCODING, DEFAULT_COMMITTEE_SIZE, TOKEN, TOKEN_DECIMALS,
};
use aleph_runtime::{
    AccountId, AuraConfig, BalancesConfig, ElectionsConfig, GenesisConfig, Perbill, SessionConfig,
    SessionKeys, StakingConfig, SudoConfig, SystemConfig, VestingConfig, WASM_BINARY,
};
use clap::Args;
use libp2p::PeerId;
use pallet_staking::{Forcing, StakerStatus};
use sc_service::{config::BasePath, ChainType};
use serde::{de::Error, Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Number, Value};
use sp_application_crypto::Ss58Codec;
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_core::{sr25519, Pair};

pub const CHAINTYPE_DEV: &str = "dev";
pub const CHAINTYPE_LOCAL: &str = "local";
pub const CHAINTYPE_LIVE: &str = "live";

pub const DEFAULT_CHAIN_ID: &str = "a0dnet1";

// Alice is the default sudo holder.
pub const DEFAULT_SUDO_ACCOUNT: &str = "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY";

pub const DEFAULT_BACKUP_FOLDER: &str = "backup-stash";

/// Specialized `ChainSpec`. This is a specialization of the general Substrate ChainSpec type.
pub type ChainSpec = sc_service::GenericChainSpec<GenesisConfig>;

#[derive(Clone)]
pub struct SerializablePeerId {
    inner: PeerId,
}

impl SerializablePeerId {
    pub fn new(inner: PeerId) -> SerializablePeerId {
        SerializablePeerId { inner }
    }
}

impl Serialize for SerializablePeerId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s: String = format!("{}", self.inner);
        serializer.serialize_str(&s)
    }
}

impl<'de> Deserialize<'de> for SerializablePeerId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let inner = PeerId::from_str(&s)
            .map_err(|_| D::Error::custom(format!("Could not deserialize as PeerId: {}", s)))?;
        Ok(SerializablePeerId { inner })
    }
}

/// Generate an account ID from seed.
pub fn account_id_from_string(seed: &str) -> AccountId {
    AccountId::from(
        sr25519::Pair::from_string(seed, None)
            .expect("Can't create pair from seed value")
            .public(),
    )
}

/// Generate AccountId based on string command line argument.
fn parse_account_id(s: &str) -> AccountId {
    AccountId::from_string(s).expect("Passed string is not a hex encoding of a public key")
}

fn parse_chaintype(s: &str) -> ChainType {
    match s {
        CHAINTYPE_DEV => ChainType::Development,
        CHAINTYPE_LOCAL => ChainType::Local,
        CHAINTYPE_LIVE => ChainType::Live,
        s => panic!("Wrong chain type {} Possible values: dev local live", s),
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct AuthorityKeys {
    pub account_id: AccountId,
    pub aura_key: AuraId,
    pub aleph_key: AlephId,
    pub peer_id: SerializablePeerId,
}

fn to_account_ids(authorities: &[AuthorityKeys]) -> impl Iterator<Item = AccountId> + '_ {
    authorities.iter().map(|auth| auth.account_id.clone())
}

#[derive(Debug, Args, Clone)]
pub struct ChainParams {
    /// Chain ID is a short identifier of the chain
    #[clap(long, value_name = "ID", default_value = DEFAULT_CHAIN_ID)]
    chain_id: String,

    /// The type of the chain. Possible values: "dev", "local", "live" (default)
    #[clap(long, value_name = "TYPE", parse(from_str = parse_chaintype), default_value = CHAINTYPE_LIVE)]
    chain_type: ChainType,

    /// Specify custom base path
    #[clap(long, short = 'd', value_name = "PATH", parse(from_os_str))]
    base_path: PathBuf,

    /// Specify filename to write node private p2p keys to
    /// Resulting keys will be stored at: base_path/account_id/node_key_file for each node
    #[clap(long, default_value = "p2p_secret")]
    node_key_file: String,

    #[clap(long, default_value = DEFAULT_BACKUP_FOLDER)]
    backup_dir: String,

    /// Chain name. Default is "Aleph Zero Development"
    #[clap(long, default_value = "Aleph Zero Development")]
    chain_name: String,

    /// Token symbol. Default is DZERO
    #[clap(long, default_value = "DZERO")]
    token_symbol: String,

    /// AccountIds of authorities forming the committee at the genesis (comma delimited)
    #[clap(long, require_value_delimiter = true, parse(from_str = parse_account_id))]
    account_ids: Vec<AccountId>,

    /// AccountId of the sudo account
    #[clap(long, parse(from_str = parse_account_id), default_value(DEFAULT_SUDO_ACCOUNT))]
    sudo_account_id: AccountId,

    /// AccountId of the optional faucet account
    #[clap(long, parse(from_str = parse_account_id))]
    faucet_account_id: Option<AccountId>,
}

impl ChainParams {
    pub fn chain_id(&self) -> &str {
        &self.chain_id
    }

    pub fn chain_type(&self) -> ChainType {
        self.chain_type.clone()
    }

    pub fn base_path(&self) -> BasePath {
        self.base_path.clone().into()
    }

    pub fn node_key_file(&self) -> &str {
        &self.node_key_file
    }

    pub fn backup_dir(&self) -> &str {
        &self.backup_dir
    }

    pub fn chain_name(&self) -> &str {
        &self.chain_name
    }

    pub fn token_symbol(&self) -> &str {
        &self.token_symbol
    }

    pub fn account_ids(&self) -> Vec<AccountId> {
        self.account_ids.clone()
    }

    pub fn sudo_account_id(&self) -> AccountId {
        self.sudo_account_id.clone()
    }

    pub fn faucet_account_id(&self) -> Option<AccountId> {
        self.faucet_account_id.clone()
    }
}

fn system_properties(token_symbol: String) -> serde_json::map::Map<String, Value> {
    [
        ("tokenSymbol".to_string(), Value::String(token_symbol)),
        (
            "tokenDecimals".to_string(),
            Value::Number(Number::from(TOKEN_DECIMALS)),
        ),
        (
            "ss58Format".to_string(),
            Value::Number(Number::from(ADDRESSES_ENCODING)),
        ),
    ]
    .iter()
    .cloned()
    .collect()
}

/// Generate chain spec for local runs.
/// Controller accounts are generated for the specified authorities.
pub fn config(
    chain_params: ChainParams,
    authorities: Vec<AuthorityKeys>,
) -> Result<ChainSpec, String> {
    let controller_accounts: Vec<AccountId> = to_account_ids(&authorities)
        .into_iter()
        .enumerate()
        .map(|(index, _account)| {
            account_id_from_string(format!("//{}//Controller", index).as_str())
        })
        .collect();
    generate_chain_spec_config(chain_params, authorities, controller_accounts)
}

fn generate_chain_spec_config(
    chain_params: ChainParams,
    authorities: Vec<AuthorityKeys>,
    controller_accounts: Vec<AccountId>,
) -> Result<ChainSpec, String> {
    let wasm_binary = WASM_BINARY.ok_or_else(|| "Development wasm not available".to_string())?;
    let token_symbol = String::from(chain_params.token_symbol());
    let chain_name = String::from(chain_params.chain_name());
    let chain_id = String::from(chain_params.chain_id());
    let chain_type = chain_params.chain_type();
    let sudo_account = chain_params.sudo_account_id();
    let faucet_account = chain_params.faucet_account_id();

    Ok(ChainSpec::from_genesis(
        // Name
        &chain_name,
        // ID
        &chain_id,
        chain_type,
        move || {
            generate_genesis_config(
                wasm_binary,
                authorities.clone(), // Initial PoA authorities, will receive funds
                sudo_account.clone(), // Sudo account, will also be pre funded
                faucet_account.clone(), // Pre-funded faucet account
                controller_accounts.clone(), // Controller accounts for staking.
            )
        },
        // Bootnodes
        vec![],
        // Telemetry
        None,
        // Protocol ID
        None,
        // Fork ID
        None,
        // Properties
        Some(system_properties(token_symbol)),
        // Extensions
        None,
    ))
}

/// Given a Vec<AccountIds> returns a unique collection
fn deduplicate(accounts: Vec<AccountId>) -> Vec<AccountId> {
    let set: HashSet<_> = accounts.into_iter().collect();
    set.into_iter().collect()
}

// total issuance of 300M (for devnet/tests/local runs only)
const TOTAL_ISSUANCE: u128 = 300_000_000u128 * 10u128.pow(TOKEN_DECIMALS);

/// Calculate initial endowments such that total issuance is kept approximately constant.
fn calculate_initial_endowment(accounts: &[AccountId]) -> u128 {
    TOTAL_ISSUANCE / (accounts.len() as u128)
}

/// Provides configuration for staking by defining balances, members, keys and stakers.
struct AccountsConfig {
    balances: Vec<(AccountId, u128)>,
    members: Vec<AccountId>,
    keys: Vec<(AccountId, AccountId, SessionKeys)>,
    stakers: Vec<(AccountId, AccountId, u128, StakerStatus<AccountId>)>,
}

/// Provides accounts for GenesisConfig setup based on distinct staking accounts.
/// Assumes validator == stash, but controller is a distinct account
fn configure_chain_spec_fields(
    unique_accounts_balances: Vec<(AccountId, u128)>,
    authorities: Vec<AuthorityKeys>,
    controllers: Vec<AccountId>,
) -> AccountsConfig {
    let balances = unique_accounts_balances
        .into_iter()
        .chain(
            controllers
                .clone()
                .into_iter()
                .map(|account| (account, TOKEN)),
        )
        .collect();

    let keys = authorities
        .iter()
        .map(|auth| {
            (
                auth.account_id.clone(),
                auth.account_id.clone(),
                SessionKeys {
                    aura: auth.aura_key.clone(),
                    aleph: auth.aleph_key.clone(),
                },
            )
        })
        .collect();

    let stakers = authorities
        .iter()
        .zip(controllers)
        .enumerate()
        .map(|(validator_idx, (validator, controller))| {
            (
                validator.account_id.clone(),
                controller,
                (validator_idx + 1) as u128 * MIN_VALIDATOR_BOND,
                StakerStatus::Validator,
            )
        })
        .collect();

    let members = to_account_ids(&authorities).collect();

    AccountsConfig {
        balances,
        members,
        keys,
        stakers,
    }
}

/// Configure initial storage state for FRAME modules.
fn generate_genesis_config(
    wasm_binary: &[u8],
    authorities: Vec<AuthorityKeys>,
    sudo_account: AccountId,
    faucet_account: Option<AccountId>,
    controller_accounts: Vec<AccountId>,
) -> GenesisConfig {
    let special_accounts = match faucet_account {
        Some(faucet_id) => vec![sudo_account.clone(), faucet_id],
        None => vec![sudo_account.clone()],
    };

    // NOTE: some combinations of bootstrap chain arguments can potentially
    // lead to duplicated rich accounts, e.g. if a sudo account is also an authority
    // which is why we remove the duplicates if any here
    let unique_accounts = deduplicate(
        to_account_ids(&authorities)
            .chain(special_accounts)
            .collect(),
    );

    let endowment = calculate_initial_endowment(&unique_accounts);

    let unique_accounts_balances = unique_accounts
        .into_iter()
        .map(|account| (account, endowment))
        .collect::<Vec<_>>();

    let validator_count = authorities.len() as u32;

    let accounts_config =
        configure_chain_spec_fields(unique_accounts_balances, authorities, controller_accounts);

    GenesisConfig {
        system: SystemConfig {
            // Add Wasm runtime to storage.
            code: wasm_binary.to_vec(),
        },
        balances: BalancesConfig {
            // Configure endowed accounts with an initial, significant balance
            balances: accounts_config.balances,
        },
        aura: AuraConfig {
            authorities: vec![],
        },
        sudo: SudoConfig {
            // Assign network admin rights.
            key: Some(sudo_account),
        },
        elections: ElectionsConfig {
            non_reserved_validators: accounts_config.members.clone(),
            committee_size: DEFAULT_COMMITTEE_SIZE,
            reserved_validators: vec![],
        },
        session: SessionConfig {
            keys: accounts_config.keys,
        },
        staking: StakingConfig {
            force_era: Forcing::NotForcing,
            validator_count,
            // to satisfy some e2e tests as this cannot be changed during runtime
            minimum_validator_count: 4,
            slash_reward_fraction: Perbill::from_percent(10),
            stakers: accounts_config.stakers,
            min_validator_bond: MIN_VALIDATOR_BOND,
            min_nominator_bond: MIN_NOMINATOR_BOND,
            ..Default::default()
        },
        treasury: Default::default(),
        vesting: VestingConfig { vesting: vec![] },
        nomination_pools: Default::default(),
        transaction_payment: Default::default(),
    }
}

pub fn mainnet_config() -> Result<ChainSpec, String> {
    ChainSpec::from_json_bytes(crate::resources::mainnet_chainspec())
}

pub fn testnet_config() -> Result<ChainSpec, String> {
    ChainSpec::from_json_bytes(crate::resources::testnet_chainspec())
}
