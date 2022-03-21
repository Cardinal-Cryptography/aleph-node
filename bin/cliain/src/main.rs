use aleph_client::{from as parse_to_protocol, KeyPair, Protocol};
use clap::{Parser, Subcommand};
use log::{error, info};
use sp_core::Pair;
use std::env;

use cliain::{
    bond, change_validators, force_new_era, prepare_keys, prompt_password_hidden, rotate_keys,
    set_keys, set_staking_limits, transfer, validate, ConnectionConfig,
};

#[derive(Debug, Parser, Clone)]
#[clap(version = "1.0")]
struct Config {
    /// WS endpoint address of the node to connect to
    #[clap(long, default_value = "127.0.0.1:9944")]
    pub node: String,

    /// Protocol to be used for connecting to node (`ws` or `wss`)
    #[clap(name = "use_ssl", parse(from_flag = parse_to_protocol))]
    pub protocol: Protocol,

    /// The seed of the key to use for signing calls
    /// If not given, an user is prompted to provide seed
    #[clap(long)]
    pub seed: Option<String>,

    /// Specific command that executes either a signed transaction or is an auxiliary command
    #[clap(subcommand)]
    pub command: Command,
}

#[derive(Debug, Clone, Subcommand)]
enum Command {
    /// Staking call to bond stash with controller
    Bond {
        /// Seed for a stash account
        #[clap(long)]
        stash_seed: String,

        /// SS58 id of the controller account
        #[clap(long)]
        controller_account: String,

        /// a Stake to bond (in tokens)
        #[clap(long)]
        initial_stake_tokens: u32,
    },

    /// Change the validator set for the session after the next
    ChangeValidators {
        /// The new validators
        #[clap(long, value_delimiter = ',')]
        validators: Vec<String>,
    },

    /// Force new era in staking world. Requires sudo.
    ForceNewEra,

    /// Associate the node with a specific staking account.
    PrepareKeys,

    /// Call rotate_keys() RPC call and prints them to stdout
    RotateKeys,

    /// Sets given keys for origin controller
    SetKeys {
        /// 64 byte hex encoded string in form 0xaabbcc..
        /// where aabbcc...  must be exactly 128 characters long
        #[clap(long)]
        new_keys: String,

        /// Seed for a controller account which signes set_keys tx
        #[clap(long)]
        controller_seed: String,
    },

    /// Command to convert given seed to SS58 Account id
    SeedToSS58,

    /// Sets lower bound for nominator and validator. Requires root account.
    SetStakingLimits {
        /// Nominator lower bound
        #[clap(long)]
        minimal_nominator_stake: u64,

        /// Validator lower bound
        #[clap(long)]
        minimal_validator_stake: u64,
    },

    /// Transfer funds via balances pallet
    Transfer {
        /// Seed of signing account
        #[clap(long)]
        from_seed: String,

        /// Number of tokens to send,
        #[clap(long)]
        amount_in_tokens: u64,

        /// SS58 id of target account
        #[clap(long)]
        to_account: String,
    },

    /// Call staking validate call for a given controller
    Validate {
        /// Seed for a controller account to intent being validator to
        #[clap(long)]
        controller_seed: String,

        /// Validator commission percentage
        #[clap(long)]
        commission_percentage: u8,
    },
}

fn main() {
    init_env();

    let Config {
        node,
        protocol,
        seed,
        command,
    } = Config::parse();

    let command_line_input_seed = match seed {
        Some(seed) => seed,
        None => match prompt_password_hidden("Provide seed for the signer account:") {
            Ok(seed) => seed,
            Err(e) => {
                error!("Failed to parse prompt with error {:?}! Exiting.", e);
                std::process::exit(1);
            }
        },
    };
    match command {
        Command::ChangeValidators { validators } => change_validators(
            ConnectionConfig::new(node, command_line_input_seed, protocol).into(),
            validators,
        ),
        Command::PrepareKeys => {
            prepare_keys(ConnectionConfig::new(node, command_line_input_seed, protocol).into())
        }
        Command::Bond {
            stash_seed,
            controller_account,
            initial_stake_tokens,
        } => bond(
            ConnectionConfig::new(node, stash_seed, protocol).into(),
            initial_stake_tokens,
            controller_account,
        ),
        Command::SetKeys {
            new_keys,
            controller_seed,
        } => set_keys(
            ConnectionConfig::new(node, controller_seed, protocol).into(),
            new_keys,
        ),
        Command::Validate {
            controller_seed,
            commission_percentage,
        } => validate(
            ConnectionConfig::new(node, controller_seed, protocol).into(),
            commission_percentage,
        ),
        Command::Transfer {
            amount_in_tokens,
            from_seed,
            to_account,
        } => transfer(
            ConnectionConfig::new(node, from_seed, protocol).into(),
            amount_in_tokens,
            to_account,
        ),
        Command::RotateKeys => {
            rotate_keys(ConnectionConfig::new(node, command_line_input_seed, protocol).into())
        }
        Command::SetStakingLimits {
            minimal_nominator_stake,
            minimal_validator_stake,
        } => set_staking_limits(
            ConnectionConfig::new(node, command_line_input_seed, protocol).into(),
            minimal_nominator_stake,
            minimal_validator_stake,
        ),
        Command::ForceNewEra => {
            force_new_era(ConnectionConfig::new(node, command_line_input_seed, protocol).into());
        }
        Command::SeedToSS58 => info!(
            "SS58 Address: {}",
            KeyPair::from_string(&command_line_input_seed, None)
                .expect("Can't create pair from seed value")
                .public()
                .to_string()
        ),
    }
}

fn init_env() {
    if env::var(env_logger::DEFAULT_FILTER_ENV).is_err() {
        env::set_var(env_logger::DEFAULT_FILTER_ENV, "info");
    }
    env_logger::init();
}
