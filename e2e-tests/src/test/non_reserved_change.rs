use crate::{
    accounts::{get_sudo_key, get_validators_keys},
    Config,
};
use aleph_client::{
    change_members, get_current_session, wait_for_finalized_block, wait_for_full_era_completion,
    wait_for_next_era, wait_for_session, AnyConnection, Header, KeyPair, RootConnection,
    SignedConnection,
};
use sp_core::Pair;
use substrate_api_client::{AccountId, XtStatus};

fn get_reserved_members(config: &Config) -> Vec<KeyPair> {
    get_validators_keys(config)[0..1].to_vec()
}

fn get_initial_non_reserved_members(config: &Config) -> Vec<KeyPair> {
    get_validators_keys(config)[1..4].to_vec()
}

fn get_new_non_reserved_members(config: &Config) -> Vec<KeyPair> {
    get_validators_keys(config)[2..].to_vec()
}

fn get_pallets_non_reserved(
    connection: &SignedConnection,
) -> anyhow::Result<(Vec<AccountId>, Vec<AccountId>)> {
    let stored_non_reserved: Vec<AccountId> = connection
        .as_connection()
        .get_storage_value("Elections", "NonReservedMembers", None)?
        .unwrap();
    let eras_members: (Vec<AccountId>, Vec<AccountId>) = connection
        .as_connection()
        .get_storage_value("Elections", "ErasMembers", None)?
        .unwrap();

    Ok((stored_non_reserved, eras_members.1))
}

pub fn change_non_reserved(config: &Config) -> anyhow::Result<()> {
    let node = &config.node;
    let accounts = get_validators_keys(config);
    let sender = accounts.first().expect("Using default accounts").to_owned();
    let connection = SignedConnection::new(node, sender);

    let sudo = get_sudo_key(config);

    let root_connection = RootConnection::new(node, sudo);

    let reserved_members: Vec<_> = get_reserved_members(config)
        .iter()
        .map(|pair| AccountId::from(pair.public()))
        .collect();

    let initial_non_reserved_members: Vec<_> = get_initial_non_reserved_members(config)
        .iter()
        .map(|pair| AccountId::from(pair.public()))
        .collect();

    let new_non_reserved_members: Vec<_> = get_new_non_reserved_members(config)
        .iter()
        .map(|pair| AccountId::from(pair.public()))
        .collect();

    change_members(
        &root_connection,
        Some(reserved_members.clone()),
        Some(initial_non_reserved_members.clone()),
        Some(4),
        XtStatus::InBlock,
    );
    wait_for_full_era_completion(&connection)?;

    change_members(
        &root_connection,
        Some(reserved_members.clone()),
        Some(new_non_reserved_members.clone()),
        Some(4),
        XtStatus::InBlock,
    );

    let current_session = get_current_session(&connection);
    wait_for_session(&connection, current_session + 1)?;

    let (stored_non_reserved, eras_non_reserved) = get_pallets_non_reserved(&connection)?;

    assert_eq!(stored_non_reserved, new_non_reserved_members);
    assert_eq!(eras_non_reserved, initial_non_reserved_members);

    wait_for_next_era(&connection)?;

    let (stored_non_reserved, eras_non_reserved) = get_pallets_non_reserved(&connection)?;

    assert_eq!(stored_non_reserved, new_non_reserved_members);
    assert_eq!(eras_non_reserved, new_non_reserved_members);

    let block_number = connection
        .as_connection()
        .get_header::<Header>(None)
        .expect("Could not fetch header")
        .expect("Block exists; qed")
        .number;
    wait_for_finalized_block(&connection, block_number)?;

    Ok(())
}
