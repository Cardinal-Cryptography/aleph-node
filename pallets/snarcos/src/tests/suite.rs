use frame_support::assert_ok;
use frame_system::{pallet_prelude::OriginFor, Config};

use super::setup::*;
use crate::{VerificationKeyIdentifier, VerificationKeys};

type Snarcos = crate::Pallet<TestRuntime>;

const IDENTIFIER: VerificationKeyIdentifier = [0; 4];
const VK: [u8; 10] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9];

fn caller() -> OriginFor<TestRuntime> {
    <TestRuntime as Config>::Origin::signed(0)
}

#[test]
fn stores_vk_with_fresh_identifier() {
    new_test_ext().execute_with(|| {
        assert_ok!(Snarcos::store_key(caller(), IDENTIFIER, VK.to_vec()));

        let stored_key = VerificationKeys::<TestRuntime>::get(IDENTIFIER);
        assert!(stored_key.is_some());
        assert_eq!(stored_key.unwrap().to_vec(), VK.to_vec());
    });
}
