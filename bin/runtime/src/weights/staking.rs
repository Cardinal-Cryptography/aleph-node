use primitives::wrap_methods;
use sp_std::marker::PhantomData;

use frame_support::dispatch::Weight;

type SubstrateStakingWeights<T> = pallet_staking::weights::SubstrateWeight<T>;

pub struct PayoutStakersDecreasedWeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_staking::WeightInfo for PayoutStakersDecreasedWeightInfo<T> {
    // To make possible to change nominators per validator we need to decrease weight for payout_stakers
    fn payout_stakers_alive_staked(n: u32) -> Weight {
        SubstrateStakingWeights::<T>::payout_stakers_alive_staked(n) / 2
    }
    wrap_methods!(
        (bond(), SubstrateStakingWeights<T>, Weight),
        (bond_extra(), SubstrateStakingWeights<T>, Weight),
        (unbond(), SubstrateStakingWeights<T>, Weight),
        (
            withdraw_unbonded_update(s: u32),
            SubstrateStakingWeights<T>,
            Weight
        ),
        (
            withdraw_unbonded_kill(s: u32),
            SubstrateStakingWeights<T>,
            Weight
        ),
        (validate(), SubstrateStakingWeights<T>, Weight),
        (kick(k: u32), SubstrateStakingWeights<T>, Weight),
        (nominate(n: u32), SubstrateStakingWeights<T>, Weight),
        (chill(), SubstrateStakingWeights<T>, Weight),
        (set_payee(), SubstrateStakingWeights<T>, Weight),
        (set_controller(), SubstrateStakingWeights<T>, Weight),
        (set_validator_count(), SubstrateStakingWeights<T>, Weight),
        (force_no_eras(), SubstrateStakingWeights<T>, Weight),
        (force_new_era(), SubstrateStakingWeights<T>, Weight),
        (force_new_era_always(), SubstrateStakingWeights<T>, Weight),
        (set_invulnerables(v: u32), SubstrateStakingWeights<T>, Weight),
        (force_unstake(s: u32), SubstrateStakingWeights<T>, Weight),
        (
            cancel_deferred_slash(s: u32),
            SubstrateStakingWeights<T>,
            Weight
        ),
        (
            payout_stakers_dead_controller(n: u32),
            SubstrateStakingWeights<T>,
            Weight
        ),
        (rebond(l: u32), SubstrateStakingWeights<T>, Weight),
        (set_history_depth(e: u32), SubstrateStakingWeights<T>, Weight),
        (reap_stash(s: u32), SubstrateStakingWeights<T>, Weight),
        (new_era(v: u32, n: u32), SubstrateStakingWeights<T>, Weight),
        (
            get_npos_voters(v: u32, n: u32, s: u32),
            SubstrateStakingWeights<T>,
            Weight
        ),
        (get_npos_targets(v: u32), SubstrateStakingWeights<T>, Weight),
        (set_staking_limits(), SubstrateStakingWeights<T>, Weight),
        (chill_other(), SubstrateStakingWeights<T>, Weight)
    );
}
