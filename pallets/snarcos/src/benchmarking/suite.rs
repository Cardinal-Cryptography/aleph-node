use frame_benchmarking::{account, benchmarks, vec};
use frame_support::traits::Get;
use frame_system::RawOrigin;

use crate::*;

const SEED: u32 = 41;

benchmarks! {

    store_key {
        let caller = account("caller", 0, SEED);
        let identifier = [0u8; 4];
        let l in 1 .. T::MaximumVerificationKeyLength::get();
        let key = vec![0u8; l as usize];
    } : _(RawOrigin::Signed(caller), identifier, key.clone())

}
