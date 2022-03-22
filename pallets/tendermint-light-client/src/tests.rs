use super::*;
use crate::{
    mock::*,
    types::{LightBlockStorage, LightClientOptionsStorage},
    utils::header_hash,
};
use frame_support::assert_ok;
use tendermint::block::{signed_header::SignedHeader, Header};
use tendermint_light_client_verifier::types::{LightBlock, TrustThreshold, ValidatorSet};

#[test]
fn successful_verification() {
    new_test_ext(|| {
        System::set_block_number(1);

        // System::initialize(
        //     &1,
        //     &System::parent_hash(),
        //     &Default::default(),
        //     Default::default(),
        // );

        Timestamp::set_timestamp(3599 * 1000);

        let origin = Origin::root();
        let options = LightClientOptionsStorage::default();
        let initial_block: LightBlockStorage = serde_json::from_str(mock::TRUSTED_BLOCK).unwrap();

        // println!("initial data {:#?}", initial_block);

        assert_ok!(Pallet::<TestRuntime>::initialize_client(
            origin,
            options.clone(),
            initial_block.clone()
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

        // let now = Timestamp::now();
        // println!("now {:#?}", now);
        // println!("header hash {:#?}", header_hash(initial_block.clone()));

        // println!("verifying block {:#?}", untrusted_block);

        let signed_header: SignedHeader = untrusted_block.signed_header.try_into().unwrap();
        let validator_set: ValidatorSet = untrusted_block.validators.try_into().unwrap();
        let trust_threshold: TrustThreshold = options.trust_threshold.try_into().unwrap();

        // let origin = Origin::signed(100);

        // assert_ok!(Pallet::<TestRuntime>::submit_finality_proof(
        //     origin,
        //     untrusted_block.clone()
        // ));
    });
}
