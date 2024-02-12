use std::env;

use aleph_client::{account_from_keypair, aleph_keypair_from_string, keypair_from_string, Pair};
use clap::Parser;
use cliain::{
    bond, call, change_validators, code_info, finalize, force_new_era, instantiate,
    instantiate_with_code, next_session_keys, nominate, prepare_keys, prompt_password_hidden,
    remove_code, rotate_keys, schedule_upgrade, set_emergency_finalizer, set_keys,
    set_staking_limits, store_key, transfer_keep_alive, treasury_approve, treasury_propose,
    treasury_reject, update_runtime, upload_code, validate, vest, vest_other, vested_transfer,
    Command, ConnectionConfig, VkStorage,
};
use log::{error, info};

#[derive(Debug, Parser, Clone)]
#[clap(version = "1.0")]
struct Config {
    /// WS endpoint address of the node to connect to
    #[clap(long, default_value = "ws://127.0.0.1:9944")]
    pub node: String,

    /// The seed of the key to use for signing calls.
    /// If not given and the command is not rpc call,
    /// a user is prompted to provide seed
    #[clap(long)]
    pub seed: Option<String>,

    /// Specific command that executes either a signed transaction or is an auxiliary command
    #[clap(subcommand)]
    pub command: Command,
}

fn read_seed(command: &Command, seed: Option<String>) -> String {
    match command {
        Command::Finalize {
            block: _,
            hash: _,
            finalizer_seed: _,
        }
        | Command::NextSessionKeys { .. }
        | Command::RotateKeys
        | Command::SeedToSS58 { .. }
        | Command::ContractCodeInfo { .. } => String::new(),
        _ => read_secret(seed, "Provide seed for the signer account:"),
    }
}

fn read_secret(secret: Option<String>, message: &str) -> String {
    match secret {
        Some(secret) => secret,
        None => match prompt_password_hidden(message) {
            Ok(secret) => secret,
            Err(e) => {
                error!("Failed to parse prompt with error {:?}! Exiting.", e);
                std::process::exit(1);
            }
        },
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_env();

    let Config {
        node,
        seed,
        command,
    } = Config::parse();

    let seed = read_seed(&command, seed);
    let cfg = ConnectionConfig::new(node, seed.clone());
    match command {
        Command::ChangeValidators {
            change_validators_args,
        } => change_validators(cfg.get_root_connection().await, change_validators_args).await,
        Command::PrepareKeys => prepare_keys(cfg.get_root_connection().await).await?,
        Command::Bond {
            initial_stake_tokens,
        } => bond(cfg.get_signed_connection().await, initial_stake_tokens).await,
        Command::Finalize {
            block,
            hash,
            finalizer_seed,
        } => {
            let finalizer_seed = read_secret(finalizer_seed, "Provide finalizer seed:");
            let finalizer = aleph_keypair_from_string(&finalizer_seed);
            finalize(cfg.get_connection().await, block, hash, finalizer).await;
        }
        Command::SetEmergencyFinalizer { finalizer_seed } => {
            let finalizer_seed = read_secret(finalizer_seed, "Provide finalizer seed:");
            let finalizer = aleph_keypair_from_string(&finalizer_seed);
            let finalizer = account_from_keypair(&finalizer);
            set_emergency_finalizer(cfg.get_root_connection().await, finalizer).await;
        }
        Command::SetKeys { new_keys } => {
            set_keys(cfg.get_signed_connection().await, new_keys).await
        }
        Command::Validate {
            commission_percentage,
        } => validate(cfg.get_signed_connection().await, commission_percentage).await,
        Command::TransferKeepAlive {
            amount_in_tokens,
            to_account,
        } => {
            transfer_keep_alive(
                cfg.get_signed_connection().await,
                amount_in_tokens,
                to_account,
            )
            .await
        }
        Command::TreasuryPropose {
            amount_in_tokens,
            beneficiary,
        } => {
            treasury_propose(
                cfg.get_signed_connection().await,
                amount_in_tokens,
                beneficiary,
            )
            .await
        }
        Command::TreasuryApprove { proposal_id } => {
            treasury_approve(cfg.get_root_connection().await, proposal_id).await
        }
        Command::TreasuryReject { proposal_id } => {
            treasury_reject(cfg.get_root_connection().await, proposal_id).await
        }
        Command::RotateKeys => rotate_keys(cfg.get_connection().await).await,
        Command::NextSessionKeys { account_id } => {
            next_session_keys(cfg.get_connection().await, account_id).await
        }
        Command::SetStakingLimits {
            minimal_nominator_stake,
            minimal_validator_stake,
            max_nominators_count,
            max_validators_count,
        } => {
            set_staking_limits(
                cfg.get_root_connection().await,
                minimal_nominator_stake,
                minimal_validator_stake,
                max_nominators_count,
                max_validators_count,
            )
            .await
        }
        Command::ForceNewEra => {
            force_new_era(cfg.get_root_connection().await).await;
        }
        Command::SeedToSS58 { input } => {
            let input = read_secret(input, "Provide seed:");
            info!(
                "SS58 Address: {}",
                keypair_from_string(&input).signer().public().to_string()
            )
        }
        Command::UpdateRuntime { runtime } => {
            update_runtime(cfg.get_root_connection().await, runtime).await
        }
        Command::Vest => vest(cfg.get_signed_connection().await).await,
        Command::VestOther { vesting_account } => {
            vest_other(cfg.get_signed_connection().await, vesting_account).await
        }
        Command::VestedTransfer {
            to_account,
            amount_in_tokens,
            per_block,
            starting_block,
        } => {
            vested_transfer(
                cfg.get_signed_connection().await,
                to_account,
                amount_in_tokens,
                per_block,
                starting_block,
            )
            .await
        }
        Command::Nominate { nominee } => nominate(cfg.get_signed_connection().await, nominee).await,
        Command::ContractInstantiateWithCode(command) => {
            match instantiate_with_code(cfg.get_signed_connection().await, command).await {
                Ok(result) => println!(
                    "{}",
                    serde_json::to_string(&result).expect("Can't encode the result as JSON")
                ),
                Err(why) => error!("Contract deployment failed {:?}", why),
            };
        }
        Command::ContractUploadCode(command) => {
            match upload_code(cfg.get_signed_connection().await, command).await {
                Ok(result) => println!("{result:?}"),
                Err(why) => error!("Contract upload failed {:?}", why),
            }
        }
        Command::ContractCall(command) => {
            match call(cfg.get_signed_connection().await, command).await {
                Ok(result) => println!("{result:?}"),
                Err(why) => error!("Contract call failed {:?}", why),
            }
        }
        Command::ContractInstantiate(command) => {
            match instantiate(cfg.get_signed_connection().await, command).await {
                Ok(result) => println!("{result:?}"),
                Err(why) => error!("Contract instantiate failed {:?}", why),
            }
        }
        Command::ContractCodeInfo(command) => {
            println!(
                "{:#?}",
                code_info(cfg.get_connection().await, command).await
            )
        }
        Command::ContractRemoveCode(command) => {
            match remove_code(cfg.get_signed_connection().await, command).await {
                Ok(result) => println!("{result:?}"),
                Err(why) => error!("Contract remove code failed {:?}", why),
            }
        }
        Command::VersionUpgradeSchedule {
            version,
            session: session_for_upgrade,
            expected_state,
        } => match schedule_upgrade(
            cfg.get_root_connection().await,
            version,
            session_for_upgrade,
            expected_state,
        )
        .await
        {
            Ok(_) => {}
            Err(why) => error!("Unable to schedule an upgrade {:?}", why),
        },

        Command::VkStorage(cmd) => match cmd {
            VkStorage::StoreKey { vk_file } => {
                if let Err(why) = store_key(cfg.get_signed_connection().await, vk_file).await {
                    error!("Unable to store key: {why:?}")
                }
            }
        },
    }
    Ok(())
}

fn init_env() {
    if env::var(env_logger::DEFAULT_FILTER_ENV).is_err() {
        env::set_var(env_logger::DEFAULT_FILTER_ENV, "info");
    }
    env_logger::init();
}
