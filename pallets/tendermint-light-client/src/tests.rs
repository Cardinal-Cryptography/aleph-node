use frame_support::assert_ok;

use super::*;
use crate::{
    mock::*,
    types::{LightBlockStorage, LightClientOptionsStorage},
};

#[test]
fn successful_verification() {
    new_test_ext(|| {
        let origin = Origin::root();
        let options = LightClientOptionsStorage::default();
        let initial_block: LightBlockStorage = serde_json::from_str(mock::TRUSTED_BLOCK).unwrap();

        println!("initial data {:#?}", initial_block);

        assert_ok!(Pallet::<TestRuntime>::initialize_client(
            origin,
            options.clone(),
            initial_block
        ));

        //  assert storage updated
        assert_eq!(Pallet::<TestRuntime>::get_options(), options.clone());

        // assert!(false);
    });
}
