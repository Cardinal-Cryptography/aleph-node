use super::*;
use crate::{
    mock::*,
    types::{LightBlockStorage, LightClientOptionsStorage},
};
use frame_support::assert_ok;

#[test]
fn successful_verification() {
    new_test_ext(|| {
        System::set_block_number(1);

        let origin = Origin::root();
        let options = LightClientOptionsStorage::default();
        let initial_block: LightBlockStorage = serde_json::from_str(mock::TRUSTED_BLOCK).unwrap();

        // println!("initial data {:#?}", initial_block);

        assert_ok!(Pallet::<TestRuntime>::initialize_client(
            origin,
            options.clone(),
            initial_block
        ));

        // assert storage updated
        assert_eq!(Pallet::<TestRuntime>::get_options(), options.clone());

        assert_eq!(Pallet::<TestRuntime>::is_halted(), false);

        System::assert_last_event(mock::Event::TendermintLightClient(
            super::Event::LightClientInitialized,
        ));

        // TODO : untrusted block
        let untrusted_block: LightBlockStorage =
            serde_json::from_str(mock::UNTRUSTED_BLOCK).unwrap();

        // println!("verifying block {:#?}", untrusted_block);

        let origin = Origin::signed(100);

        // TODO : adjust now time

        assert_ok!(Pallet::<TestRuntime>::submit_finality_proof(
            origin,
            untrusted_block
        ));
    });
}
