#![cfg(feature = "runtime-benchmarks")]

use super::*;

use frame_benchmarking::{account, benchmarks_instance_pallet, whitelisted_caller};
use frame_system::RawOrigin;
use sp_runtime::traits::Bounded;

use crate::Pallet as TendermintLightClient;

const SEED: u32 = 0;
// // existential deposit multiplier
// const ED_MULTIPLIER: u32 = 10;

benchmarks_instance_pallet! {
    // Benchmark `transfer` extrinsic with the worst possible conditions:
    // * Transfer will kill the sender account.
    // * Transfer will create the recipient account.
    transfer {

    }: transfer(RawOrigin::Signed(caller.clone()), recipient_lookup, transfer_amount)
    verify {
        // assert_eq!(Balances::<T, I>::free_balance(&recipient), transfer_amount);
    }


}
