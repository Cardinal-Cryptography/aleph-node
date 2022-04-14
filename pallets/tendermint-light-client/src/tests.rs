use super::*;
use crate::{
    mock::*,
    types::{
        LightBlockStorage, LightClientOptionsStorage, TendermintHashStorage, TimestampStorage,
    },
};
use frame_support::{assert_err, assert_ok};
use tendermint_light_client_verifier::types::LightBlock;

#[test]
fn type_casting() {
    let light_block: LightBlock = serde_json::from_str(mock::TRUSTED_BLOCK).unwrap();
    let light_block_storage: LightBlockStorage = serde_json::from_str(mock::TRUSTED_BLOCK).unwrap();
    let light_block_from_storage: LightBlock = light_block_storage.clone().try_into().unwrap();

    assert_eq!(light_block, light_block_from_storage);
}

#[test]
fn type_casting_is_commutative() {
    let light_block: LightBlock = serde_json::from_str(mock::TRUSTED_BLOCK).unwrap();
    let light_block_storage: LightBlockStorage = light_block.clone().try_into().unwrap();
    let light_block_from_storage: LightBlock = light_block_storage.clone().try_into().unwrap();

    assert_eq!(light_block, light_block_from_storage);
}

#[test]
fn block_generation() {
    let mut blocks = crate::generator::generate_consecutive_blocks(
        1,
        String::from("test-chain"),
        1,
        3,
        TimestampStorage::new(3, 0),
    );

    let light_block_storage: LightBlockStorage = blocks.pop().unwrap();
    let light_block_from_storage: LightBlock = light_block_storage.clone().try_into().unwrap();
    let light_block: LightBlockStorage = light_block_from_storage.clone().try_into().unwrap();

    assert_eq!(light_block, light_block_storage);
}

#[test]
fn successful_verification() {
    new_test_ext(|| {
        System::set_block_number(1);
        // 1970-01-01T00:00:05Z
        Timestamp::set_timestamp(5000);

        let root = Origin::root();
        let options = LightClientOptionsStorage::default();
        let initial_block: LightBlockStorage = serde_json::from_str(mock::TRUSTED_BLOCK).unwrap();

        assert_ok!(Pallet::<TestRuntime>::initialize_client(
            root,
            options.clone(),
            initial_block.clone()
        ));

        // assert storage updated
        assert_eq!(Pallet::<TestRuntime>::get_options(), options.clone());

        assert_eq!(Pallet::<TestRuntime>::is_halted(), false);

        let last_imported = Pallet::<TestRuntime>::get_last_imported_block_hash();
        assert_eq!(
            initial_block.signed_header.commit.block_id.hash,
            TendermintHashStorage::Some(last_imported)
        );

        System::assert_last_event(mock::Event::TendermintLightClient(
            super::Event::LightClientInitialized,
        ));

        let untrusted_block: LightBlockStorage =
            serde_json::from_str(mock::UNTRUSTED_BLOCK).unwrap();

        let origin = Origin::signed(100);

        assert_ok!(Pallet::<TestRuntime>::update_client(
            origin,
            untrusted_block.clone()
        ));

        let best_finalized_hash = Pallet::<TestRuntime>::get_last_imported_block_hash();
        assert_eq!(
            untrusted_block.signed_header.commit.block_id.hash,
            TendermintHashStorage::Some(best_finalized_hash)
        );

        System::assert_last_event(mock::Event::TendermintLightClient(
            super::Event::ImportedLightBlock(100, best_finalized_hash),
        ));

        // println!("@ chain-id {:?}", String::from("test-chain").as_bytes());

        let last_imported_block = Pallet::<TestRuntime>::get_last_imported_block().unwrap();
        assert_eq!(untrusted_block, last_imported_block);
    });
}

#[test]
fn failed_verification() {
    new_test_ext(|| {
        System::set_block_number(1);

        let options = LightClientOptionsStorage::default();

        // 1970-01-01T00:00:03Z + trusting period + clock_drift
        Timestamp::set_timestamp(3000 + (options.trusting_period + options.clock_drift) * 1000);

        let root = Origin::root();
        let initial_block: LightBlockStorage = serde_json::from_str(mock::TRUSTED_BLOCK).unwrap();

        assert_ok!(Pallet::<TestRuntime>::initialize_client(
            root,
            options.clone(),
            initial_block.clone()
        ));

        let untrusted_block: LightBlockStorage =
            serde_json::from_str(mock::UNTRUSTED_BLOCK).unwrap();

        let origin = Origin::signed(100);

        assert_err!(
            Pallet::<TestRuntime>::update_client(origin, untrusted_block.clone()),
            super::Error::<TestRuntime>::InvalidBlock
        );
    });
}

// TODO : round_robin_storage test

#[test]
fn halted() {
    new_test_ext(|| {
        System::set_block_number(1);

        let options = LightClientOptionsStorage::default();

        // 1970-01-01T00:00:03Z + trusting period + clock_drift
        Timestamp::set_timestamp(3000 + (options.trusting_period + options.clock_drift) * 1000);

        let root = Origin::root();
        let initial_block: LightBlockStorage = serde_json::from_str(mock::TRUSTED_BLOCK).unwrap();

        assert_ok!(Pallet::<TestRuntime>::initialize_client(
            root.clone(),
            options.clone(),
            initial_block.clone()
        ));

        assert_ok!(Pallet::<TestRuntime>::set_halted(root, true));

        let untrusted_block: LightBlockStorage =
            serde_json::from_str(mock::UNTRUSTED_BLOCK).unwrap();

        let origin = Origin::signed(100);

        assert_err!(
            Pallet::<TestRuntime>::update_client(origin, untrusted_block.clone()),
            super::Error::<TestRuntime>::Halted
        );
    });
}
