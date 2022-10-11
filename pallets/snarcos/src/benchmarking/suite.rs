use frame_benchmarking::{account, benchmarks, vec};
use frame_support::{traits::Get, BoundedVec};
use frame_system::RawOrigin;

use crate::*;

const SEED: u32 = 41;

fn insert_keys<T: Config>(number_of_keys: u32) -> Result<(), &'static str> {
    let key = vec![0u8; 8]; // short, universal key
    for i in 0..number_of_keys {
        VerificationKeys::<T>::insert(i.to_le_bytes(), BoundedVec::try_from(key.clone()).unwrap());
    }
    Ok(())
}

benchmarks! {

    store_key {
        let a in 0 .. T::MaxKeys::get(); => insert_keys::<T>(a)?; // how many keys are already stored
        let l in 1 .. T::MaxVerificationKeyLength::get();         // key length

        let key = vec![0u8; l as usize];                          // key itself

        let identifier = [255u8; 4];                              // new identifier, unused yet
        let caller = account("caller", 0, SEED);                  // we can use arbitrary account

    } : _(RawOrigin::Signed(caller), identifier, key.clone())

}
