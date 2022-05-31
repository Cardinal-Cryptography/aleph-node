use aleph_client::BlockNumber;
use clap::Subcommand;
use primitives::Balance;
use sp_core::H256;
use std::path::PathBuf;
use substrate_api_client::AccountId;

#[derive(Debug, Clone, Subcommand)]
pub enum Command {
    /// Staking call to bond stash with controller
    Bond {
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

    /// Declare the desire to nominate target account
    Nominate {
        #[clap(long)]
        nominee: String,
    },

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

        /// Maximum number of nominators
        #[clap(long)]
        max_nominators_count: Option<u32>,

        /// Maximum number of validators
        #[clap(long)]
        max_validators_count: Option<u32>,
    },

    /// Transfer funds via balances pallet
    Transfer {
        /// Number of tokens to send,
        #[clap(long)]
        amount_in_tokens: u64,

        /// SS58 id of target account
        #[clap(long)]
        to_account: String,
    },

    /// Send new runtime (requires sudo account)
    UpdateRuntime {
        #[clap(long)]
        /// Path to WASM file with runtime
        runtime: String,
    },

    /// Call staking validate call for a given controller
    Validate {
        /// Validator commission percentage
        #[clap(long)]
        commission_percentage: u8,
    },

    /// Update vesting for the calling account.
    Vest,

    /// Update vesting on behalf of the given account.
    VestOther {
        /// Account seed for which vesting should be performed.
        #[clap(long)]
        vesting_account: String,
    },

    /// Transfer funds via balances pallet
    VestedTransfer {
        /// Number of tokens to send.
        #[clap(long)]
        amount_in_tokens: u64,

        /// Seed of the target account.
        #[clap(long)]
        to_account: String,

        /// How much balance (in rappens, not in tokens) should be unlocked per block.
        #[clap(long)]
        per_block: Balance,

        /// Block number when unlocking should start.
        #[clap(long)]
        starting_block: BlockNumber,
    },

    /// Print debug info of storage
    DebugStorage,

    /// Uploads new code without instantiating a contract from it
    /// https://polkadot.js.org/docs/substrate/extrinsics/#uploadcodecode-bytes-storage_deposit_limit-optioncompactu128
    ContractUploadCode {
        /// Path to the .wasm artifact
        #[clap(long, parse(from_os_str))]
        wasm_path: PathBuf,
        /// The maximum amount of balance that can be charged/reserved from the caller to pay for the storage consumed
        #[clap(long)]
        storage_deposit_limit: Option<u128>,
    },

    ///  Instantiates a contract from a previously deployed wasm binary.
    /// API signature: https://polkadot.js.org/docs/substrate/extrinsics/#instantiatevalue-compactu128-gas_limit-compactu64-storage_deposit_limit-optioncompactu128-code_hash-h256-data-bytes-salt-bytes
    ContractInstantiate {
        /// balance to transfer from the call origin to the contract
        #[clap(long, default_value = "0")]
        balance: u128,
        /// The gas limit enforced when executing the constructor
        #[clap(long, default_value = "1000000000")]
        gas_limit: u64,
        /// The maximum amount of balance that can be charged/reserved from the caller to pay for the storage consumed
        #[clap(long)]
        storage_deposit_limit: Option<u128>,
        /// Path to the .wasm artifact
        #[clap(long, parse(from_os_str))]
        metadata_path: PathBuf,
        /// Code hash of the deployed contract
        #[clap(long, parse(try_from_str))]
        code_hash: H256,
        /// The name of the contract constructor to call
        #[clap(name = "constructor", long, default_value = "new")]
        constructor: String,
        /// The constructor arguments, encoded as strings
        #[clap(long, multiple_values = true)]
        args: Option<Vec<String>>,
    },

    /// Deploys a new contract, returns its code hash and the AccountId of the instance
    /// contract cannot already exist on-chain
    /// API signature: https://polkadot.js.org/docs/substrate/extrinsics/#instantiatewithcodevalue-compactu128-gas_limit-compactu64-storage_deposit_limit-optioncompactu128-code-bytes-data-bytes-salt-bytes
    ContractInstantiateWithCode {
        /// Path to the .wasm artifact
        #[clap(long, parse(from_os_str))]
        wasm_path: PathBuf,
        /// Path to the .json file with contract metadata (abi)
        #[clap(long, parse(from_os_str))]
        metadata_path: PathBuf,
        /// The name of the contract constructor to call
        #[clap(name = "constructor", long, default_value = "new")]
        constructor: String,
        /// The constructor arguments, encoded as strings, space separated
        #[clap(long, multiple_values = true)]
        args: Option<Vec<String>>,
        /// balance to transfer from the origin to the newly created contract
        #[clap(long, default_value = "0")]
        balance: u128,
        /// The gas limit enforced when executing the constructor
        #[clap(long, default_value = "1000000000")]
        gas_limit: u64,
        /// The maximum amount of balance that can be charged/reserved from the caller to pay for the storage consumed
        #[clap(long)]
        storage_deposit_limit: Option<u128>,
    },

    /// Calls a contract
    /// API signature: https://polkadot.js.org/docs/substrate/extrinsics/#calldest-multiaddress-value-compactu128-gas_limit-compactu64-storage_deposit_limit-optioncompactu128-data-bytes
    ContractCall {
        /// Address of the contract to call
        #[clap(long, parse(try_from_str))]
        destination: AccountId,
        /// Path to the .json fiel with contract metadata (abi)
        #[clap(long, parse(from_os_str))]
        metadata_path: PathBuf,
        /// balance to transfer from the call origin to the contract
        #[clap(long, default_value = "0")]
        balance: u128,
        /// The gas limit enforced when executing the constructor
        #[clap(long, default_value = "1000000000")]
        gas_limit: u64,
        /// The maximum amount of balance that can be charged/reserved from the caller to pay for the storage consumed
        #[clap(long)]
        storage_deposit_limit: Option<u128>,
        /// The name of the contract message to call
        #[clap(long)]
        message: String,
        /// The message arguments, encoded as strings
        #[clap(long, multiple_values = true)]
        args: Option<Vec<String>>,
    },

    /// Remove the code stored under code_hash and refund the deposit to its owner.
    /// Code can only be removed by its original uploader (its owner) and only if it is not used by any contract.
    /// API signature: https://polkadot.js.org/docs/substrate/extrinsics/#removecodecode_hash-h256
    ContractRemoveCode {
        /// Code hash of the deployed contract
        #[clap(long, parse(try_from_str))]
        code_hash: H256,
    },
}
