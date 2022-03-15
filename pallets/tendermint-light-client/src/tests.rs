use super::*;
use crate::{
    mock::*,
    types::{LightBlockStorage, LightClientOptionsStorage},
};

#[test]
fn successful_verification() {
    new_test_ext(|| {
        // TODO: initial trusted block

        let origin = Origin::root();
        let options = LightClientOptionsStorage::default();

        let initial_block: LightBlockStorage = serde_json::from_str(mock::TRUSTED_BLOCK).unwrap();

        println!("initial data {:?}", initial_block);

        // let call = Pallet::<TestRuntime>::initialize_client(Origin::root(), options, initial_block);

        assert!(true);
    });
}
