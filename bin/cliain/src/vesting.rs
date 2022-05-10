use aleph_client::{
    account_from_keypair, keypair_from_string, BlockNumber, Connection, KeyPair, VestingSchedule,
};
use log::{error, info};
use primitives::{Balance, TOKEN};

fn get_caller(connection: &Connection) -> KeyPair {
    connection
        .signer
        .clone()
        .expect("Connection should be signed")
}

pub fn vest(connection: Connection) {
    let caller = get_caller(&connection);
    match aleph_client::vest(&connection, caller) {
        Ok(_) => info!("Vesting has succeeded"),
        Err(e) => error!("Vesting has failed with:\n {:?}", e),
    }
}

pub fn vest_other(connection: Connection, vesting_account_seed: String) {
    let caller = get_caller(&connection);
    let vester = account_from_keypair(&keypair_from_string(vesting_account_seed.as_str()));
    match aleph_client::vest_other(&connection, caller, vester) {
        Ok(_) => info!("Vesting on behalf has succeeded"),
        Err(e) => error!("Vesting on behalf has failed with:\n {:?}", e),
    }
}

pub fn vested_transfer(
    connection: Connection,
    target_seed: String,
    amount_in_tokens: u64,
    per_block: Balance,
    starting_block: BlockNumber,
) {
    let sender = get_caller(&connection);
    let receiver = account_from_keypair(&keypair_from_string(target_seed.as_str()));
    let schedule =
        VestingSchedule::new(amount_in_tokens as u128 * TOKEN, per_block, starting_block);
    match aleph_client::vested_transfer(&connection, sender, receiver, schedule) {
        Ok(_) => info!("Vested transfer has succeeded"),
        Err(e) => error!("Vested transfer has failed with:\n {:?}", e),
    }
}
