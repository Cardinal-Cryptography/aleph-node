use std::collections::HashMap;

use aleph_primitives::{
    AuthorityId as AlephId, DEFAULT_MILLISECS_PER_BLOCK, DEFAULT_SESSION_PERIOD,
};
use aleph_runtime::{
    AccountId, AlephConfig, AuraConfig, BalancesConfig, GenesisConfig, SessionConfig, SessionKeys,
    Signature, SudoConfig, SystemConfig, WASM_BINARY,
};
use hex_literal::hex;
use sc_service::ChainType;
use sp_application_crypto::key_types;
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_core::{ed25519, sr25519, Pair, Public};
use sp_runtime::traits::{IdentifyAccount, Verify};
use std::{env::VarError, fmt::Display, str::FromStr};

const SESSION_PERIOD_ENV_VAR: &str = "SESSION_PERIOD";
const MILLISECS_PER_BLOCK_ENV_VAR: &str = "MILLISECS_PER_BLOCK";

const FAUCET_HASH: [u8; 32] =
    hex!("eaefd9d9b42915bda608154f17bb03e407cbf244318a0499912c2fb1cd879b74");

pub(crate) const LOCAL_AUTHORITIES: [&str; 8] = [
    "Damian", "Tomasz", "Zbyszko", "Hansu", "Adam", "Matt", "Antoni", "Michal",
];

pub(crate) const KEY_PATH: &str = "/tmp/authorities_keys";

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
pub fn get_account_id_from_seed<TPublic: Public>(seed: &&str) -> AccountId
where
    AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
    AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

#[derive(Clone)]
struct AuthorityKeys {
    account_id: AccountId,
    aura_key: AuraId,
    aleph_key: AlephId,
}

fn read_keys(n_members: usize) -> Vec<AuthorityKeys> {
    let auth_keys: HashMap<u32, Vec<[u8; 32]>> =
        if let Ok(auth_keys) = std::fs::read_to_string(KEY_PATH) {
            serde_json::from_str(&auth_keys).expect("should contain list of keys")
        } else {
            return Default::default();
        };

    let aura_keys = auth_keys
        .get(&key_types::AURA.into())
        .unwrap()
        .iter()
        .copied()
        .map(|bytes| AuraId::from(sr25519::Public::from_raw(bytes)));

    let aleph_keys = auth_keys
        .get(&aleph_primitives::KEY_TYPE.into())
        .unwrap()
        .iter()
        .copied()
        .map(|bytes| AlephId::from(ed25519::Public::from_raw(bytes)));

    let account_ids = LOCAL_AUTHORITIES
        .iter()
        .map(get_account_id_from_seed::<sr25519::Public>);

    aura_keys
        .zip(aleph_keys)
        .zip(account_ids)
        .take(n_members)
        .map(|((aura_key, aleph_key), account_id)| AuthorityKeys {
            aura_key,
            aleph_key,
            account_id,
        })
        .collect()
}

// TODO: rename to AlephEnvVarParams
#[derive(Clone, Copy)]
struct EnvironmentVariables {
    session_period: u32,
    millisecs_per_block: u64,
}

impl EnvironmentVariables {
    fn fetch() -> Result<Self, String> {
        Ok(Self {
            session_period: Self::fetch_var_or(SESSION_PERIOD_ENV_VAR, DEFAULT_SESSION_PERIOD)?,
            millisecs_per_block: Self::fetch_var_or(
                MILLISECS_PER_BLOCK_ENV_VAR,
                DEFAULT_MILLISECS_PER_BLOCK,
            )?,
        })
    }
    fn fetch_var_or<T>(var: &str, default: T) -> Result<T, String>
    where
        T: FromStr + Display,
        <T as FromStr>::Err: ToString,
    {
        match std::env::var(var) {
            Ok(value) => match value.parse() {
                Ok(value) => Ok(value),
                Err(err) => Err(err.to_string()),
            },
            Err(VarError::NotPresent) => {
                log::info!("env var {} missing, using default value {}", var, default);
                Ok(default)
            }
            Err(err) => Err(err.to_string()),
        }
    }
}

pub fn development_config() -> Result<ChainSpec, String> {
    let wasm_binary = WASM_BINARY.ok_or_else(|| "Development wasm not available".to_string())?;

    let n_members = std::fs::read_to_string("/tmp/n_members")
        .expect("Committee size is not specified")
        .trim()
        .parse::<usize>()
        .expect("Wrong committee size");

    let authorities = read_keys(n_members);

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
    .map(get_account_id_from_seed::<sr25519::Public>)
    // Also give money to the faucet account.
    .chain(std::iter::once(FAUCET_HASH.into()))
    .collect();

    let sudo_account = rich_accounts[0].clone();
    let env_vars = EnvironmentVariables::fetch()?;

    Ok(ChainSpec::from_genesis(
        // Name
        "AlephZero Development",
        // ID
        "dev",
        ChainType::Development,
        move || {
            testnet_genesis(
                wasm_binary,
                // Initial PoA authorities
                authorities.clone(),
                // Pre-funded accounts
                sudo_account.clone(),
                rich_accounts.clone(),
                env_vars,
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

pub fn testnet1_config() -> Result<ChainSpec, String> {
    let wasm_binary = WASM_BINARY.ok_or_else(|| "Development wasm not available".to_string())?;

    let n_members = 6;
    let authorities = read_keys(n_members);

    let sudo_public: sr25519::Public = authorities[0].aura_key.clone().into();
    let sudo_account: AccountId = AccountPublic::from(sudo_public).into_account();

    // Give money to the faucet account.
    let faucet: AccountId = FAUCET_HASH.into();
    let rich_accounts = vec![faucet];

    let env_vars = EnvironmentVariables::fetch()?;
    Ok(ChainSpec::from_genesis(
        // Name
        "Aleph Zero",
        // ID
        "a0tnet1",
        ChainType::Live,
        move || {
            testnet_genesis(
                wasm_binary,
                authorities.clone(),
                sudo_account.clone(),
                // Pre-funded accounts
                rich_accounts.clone(),
                env_vars,
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
    env_vars: EnvironmentVariables,
) -> GenesisConfig {
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
            session_period: env_vars.session_period,
            millisecs_per_block: env_vars.millisecs_per_block,
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
    }
}
