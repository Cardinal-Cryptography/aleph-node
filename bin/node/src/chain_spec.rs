use aleph_primitives::{
    AuthorityId as AlephId, DEFAULT_MILLISECS_PER_BLOCK, DEFAULT_SESSION_PERIOD,
};
use aleph_runtime::{
    AccountId, AlephConfig, AuraConfig, BalancesConfig, GenesisConfig, SessionConfig, SessionKeys,
    Signature, SudoConfig, SystemConfig, VestingConfig, WASM_BINARY,
};
use hex_literal::hex;
use sc_service::ChainType;
use serde::{Deserialize, Serialize};
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_core::{sr25519, Pair, Public};
use sp_runtime::traits::{IdentifyAccount, Verify};
use std::{env::VarError, fmt::Display, str::FromStr};

const FAUCET_HASH: [u8; 32] =
    hex!("eaefd9d9b42915bda608154f17bb03e407cbf244318a0499912c2fb1cd879b74");

pub const LOCAL_AUTHORITIES: [&str; 8] = [
    "Damian", "Tomasz", "Zbyszko", "Hansu", "Adam", "Matt", "Antoni", "Michal",
];

pub(crate) const TESTNET_ID: &str = "a0tnet1";
pub(crate) const DEVNET_ID: &str = "dev";

/// Specialized `ChainSpec`. This is a specialization of the general Substrate ChainSpec type.
pub type ChainSpec = sc_service::GenericChainSpec<GenesisConfig>;

/// Generate a crypto pair from seed.
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
    TPublic::Pair::from_string(&format!("//{}", seed), None)
        .expect("static values are valid; qed")
        .public()
}

type AccountPublic = <Signature as Verify>::Signer;

/// Generate an account ID from seed.
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
    AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
    AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AuthorityKeys {
    pub account_id: AccountId,
    pub aura_key: AuraId,
    pub aleph_key: AlephId,
}

#[derive(Clone)]
pub struct ChainParams {
    session_period: u32,
    millisecs_per_block: u64,
    keys_path: String,
}

impl ChainParams {
    pub fn from_cli(
        session_period: Option<u32>,
        millisecs_per_block: Option<u64>,
        keys_path: String,
    ) -> Self {
        ChainParams {
            session_period: Self::param(
                "session period",
                session_period,
                "SESSION_PERIOD",
                DEFAULT_SESSION_PERIOD,
            ),
            millisecs_per_block: Self::param(
                "millisecs per block",
                millisecs_per_block,
                "MILLISECS_PER_BLOCK",
                DEFAULT_MILLISECS_PER_BLOCK,
            ),
            keys_path,
        }
    }

    fn param<T: FromStr + Display>(
        debug_name: &str,
        cli_value: Option<T>,
        var: &str,
        default: T,
    ) -> T
    where
        <T as FromStr>::Err: ToString,
    {
        cli_value
            .or_else(|| Self::parse_env_var(var))
            .unwrap_or_else(|| {
                log::debug!(
                    "{} parameter not specified, using default value: {}",
                    debug_name,
                    default
                );
                default
            })
    }

    fn parse_env_var<T: FromStr>(var: &str) -> Option<T>
    where
        <T as FromStr>::Err: ToString,
    {
        match std::env::var(var) {
            Ok(value) => match value.parse() {
                Ok(value) => Some(value),
                Err(err) => {
                    panic!(
                        "error parsing environment variable {}: {}",
                        var,
                        err.to_string()
                    );
                }
            },
            Err(VarError::NotPresent) => None,
            Err(err @ VarError::NotUnicode(_)) => {
                panic!("environment variable {} is not unicode: {}", var, err);
            }
        }
    }
}

fn read_keys(keys_path: String) -> Vec<AuthorityKeys> {
    if let Ok(auth_keys) = std::fs::read_to_string(keys_path) {
        serde_json::from_str(&auth_keys).expect("should contain list of keys")
    } else {
        Vec::new()
    }
}

pub fn development_config(mut chain_params: ChainParams) -> Result<ChainSpec, String> {
    let wasm_binary = WASM_BINARY.ok_or_else(|| "Development wasm not available".to_string())?;

    let authorities = read_keys(std::mem::take(&mut chain_params.keys_path));

    let rich_accounts: Vec<_> = [
        "Alice",
        "Alice//stash",
        "Bob",
        "Bob//stash",
        "Charlie",
        "Dave",
        "Eve",
    ]
    .iter()
    .map(|account| get_account_id_from_seed::<sr25519::Public>(*account))
    // Also give money to the faucet account.
    .chain(std::iter::once(FAUCET_HASH.into()))
    .collect();

    let sudo_account = rich_accounts[0].clone();

    Ok(ChainSpec::from_genesis(
        // Name
        "AlephZero Development",
        // ID
        DEVNET_ID,
        ChainType::Development,
        move || {
            testnet_genesis(
                wasm_binary,
                // Initial PoA authorities
                authorities.clone(),
                // Pre-funded accounts
                sudo_account.clone(),
                rich_accounts.clone(),
                chain_params.clone(),
            )
        },
        // Bootnodes
        vec![],
        // Telemetry
        None,
        // Protocol ID
        None,
        // Properties
        Some(
            [(
                "tokenSymbol".to_string(),
                serde_json::Value::String("DZERO".into()),
            )]
            .iter()
            .cloned()
            .collect(),
        ),
        // Extensions
        None,
    ))
}

pub fn testnet1_config(mut chain_params: ChainParams) -> Result<ChainSpec, String> {
    let wasm_binary = WASM_BINARY.ok_or_else(|| "Development wasm not available".to_string())?;

    let authorities = read_keys(std::mem::take(&mut chain_params.keys_path));

    let sudo_public: sr25519::Public = authorities[0].aura_key.clone().into();
    let sudo_account: AccountId = AccountPublic::from(sudo_public).into_account();

    // Give money to the faucet account.
    let faucet: AccountId = FAUCET_HASH.into();
    let rich_accounts = vec![faucet];
    Ok(ChainSpec::from_genesis(
        // Name
        "Aleph Zero",
        // ID
        TESTNET_ID,
        ChainType::Live,
        move || {
            testnet_genesis(
                wasm_binary,
                authorities.clone(),
                sudo_account.clone(),
                // Pre-funded accounts
                rich_accounts.clone(),
                chain_params.clone(),
            )
        },
        // Bootnodes
        vec![],
        // Telemetry
        None,
        // Protocol ID
        None,
        // Properties
        Some(
            [(
                "tokenSymbol".to_string(),
                serde_json::Value::String("TZERO".into()),
            )]
            .iter()
            .cloned()
            .collect(),
        ),
        // Extensions
        None,
    ))
}

/// Configure initial storage state for FRAME modules.
fn testnet_genesis(
    wasm_binary: &[u8],
    authorities: Vec<AuthorityKeys>,
    root_key: AccountId,
    rich_accounts: Vec<AccountId>,
    chain_params: ChainParams,
) -> GenesisConfig {
    let session_period = chain_params.session_period;
    let millisecs_per_block = chain_params.millisecs_per_block;
    log::debug!(
        "session-period: {}, millisecs-per-block: {}",
        session_period,
        millisecs_per_block
    );
    GenesisConfig {
        system: SystemConfig {
            // Add Wasm runtime to storage.
            code: wasm_binary.to_vec(),
            changes_trie_config: Default::default(),
        },
        balances: BalancesConfig {
            // Configure endowed accounts with initial balance of 1 << 60.
            balances: authorities
                .iter()
                .map(|auth| &auth.account_id)
                .cloned()
                .chain(rich_accounts.into_iter())
                .map(|k| (k, 1 << 60))
                .collect(),
        },
        aura: AuraConfig {
            authorities: vec![],
        },
        sudo: SudoConfig {
            // Assign network admin rights.
            key: root_key,
        },
        aleph: AlephConfig {
            authorities: authorities
                .iter()
                .map(|auth| auth.aleph_key.clone())
                .collect(),
            session_period,
            millisecs_per_block,
        },
        session: SessionConfig {
            keys: authorities
                .into_iter()
                .map(|auth| {
                    (
                        auth.account_id.clone(),
                        auth.account_id.clone(),
                        SessionKeys {
                            aura: auth.aura_key.clone(),
                            aleph: auth.aleph_key,
                        },
                    )
                })
                .collect(),
        },
        vesting: VestingConfig { vesting: vec![] },
    }
}
