pub use nomination_pools::CustomMigrateToV2;

mod nomination_pools {
    use codec::{Decode, Encode, Error, Input, MaxEncodedLen};
    use frame_support::{
        dispatch::TypeInfo,
        log,
        traits::{OnRuntimeUpgrade, StorageVersion},
        RuntimeDebugNoBound,
    };
    use pallet_nomination_pools::{
        BalanceOf, BondedPools, Config, Metadata, Pallet, PoolId, PoolMember, PoolMembers,
        ReversePoolIdLookup, RewardPool, RewardPools, SubPoolsStorage,
    };
    use sp_core::{Get, U256};
    use sp_staking::EraIndex;
    use sp_std::{collections::btree_set::BTreeSet, prelude::*};

    use crate::sp_api_hidden_includes_construct_runtime::hidden_include::dispatch::GetStorageVersion; // sick
    use crate::Weight;

    #[derive(Decode)]
    pub struct OldRewardPool<B> {
        pub balance: B,
        pub total_earnings: B,
        pub points: U256,
    }

    #[derive(Encode, Decode, MaxEncodedLen, TypeInfo, RuntimeDebugNoBound)]
    pub struct OldPoolMember<T: Config> {
        pub pool_id: PoolId,
        pub points: BalanceOf<T>,
        pub reward_pool_total_earnings: BalanceOf<T>,
        pub unbonding_eras:
            sp_core::bounded::BoundedBTreeMap<EraIndex, BalanceOf<T>, T::MaxUnbonding>,
    }

    enum EitherRewardPool<T: Config, B> {
        Old(OldRewardPool<B>),
        New(RewardPool<T>),
    }

    impl<T: Config, B: Decode> Decode for EitherRewardPool<T, B> {
        fn decode<I: Input>(input: &mut I) -> Result<Self, Error> {
            let len = input.remaining_len()?.unwrap_or_default();
            let mut buffer = vec![0; len];
            input.read(&mut buffer)?;
            if let Ok(new) = OldRewardPool::<B>::decode(&mut buffer.clone().as_slice()) {
                return Ok(EitherRewardPool::Old(new));
            }

            RewardPool::<T>::decode(&mut buffer.as_slice()).map(|old| EitherRewardPool::New(old))
        }
    }

    fn dissolve_pool<T: Config>(id: PoolId) {
        let bonded_account = Pallet::<T>::create_bonded_account(id);
        ReversePoolIdLookup::<T>::remove(&bonded_account);
        SubPoolsStorage::<T>::remove(id);
        Metadata::<T>::remove(id);
        BondedPools::<T>::remove(id)
    }

    /// Delete pools, members and their bonded pool in the old scheme
    /// <https://github.com/paritytech/substrate/pull/11669.>.
    pub struct CustomMigrateToV2<T>(sp_std::marker::PhantomData<T>);

    impl<T: Config> CustomMigrateToV2<T> {
        fn run() -> Weight {
            let mut old_ids = BTreeSet::new();

            // delete old pools
            RewardPools::<T>::translate::<EitherRewardPool<T, BalanceOf<T>>, _>(|key, either| {
                match either {
                    EitherRewardPool::Old(_) => {
                        old_ids.insert(key);
                        log::debug!(target: "runtime::nomination-pools", "deleting pool with id {}", key);
                        dissolve_pool::<T>(key);
                        None
                    }
                    EitherRewardPool::New(new) => Some(new),
                }
            });

            PoolMembers::<T>::translate::<PoolMember<T>, _>(|key, member: PoolMember<T>| {
                if !old_ids.contains(&member.pool_id) {
                    return Some(member);
                }

                log::debug!(target: "runtime::nomination-pools", "deleting member {:?}", key.encode());
                None
            });

            log::debug!(target: "runtime::nomination-pools", "deleted pools {:?}", old_ids);
            StorageVersion::new(2).put::<Pallet<T>>();

            T::DbWeight::get().reads(1)
        }
    }

    impl<T: Config> OnRuntimeUpgrade for CustomMigrateToV2<T> {
        fn on_runtime_upgrade() -> Weight {
            let current = Pallet::<T>::current_storage_version();
            let onchain = Pallet::<T>::on_chain_storage_version();

            log::info!(target: "runtime::nomination-pools",
                "Running migration with current storage version {:?} / onchain {:?}",
                current,
                onchain
            );

            //on testnet we have storage_version set to 0
            if onchain == 0 {
                Self::run()
            } else {
                log::info!(target: "runtime::nomination-pools",
                    "MigrateToV2 did not executed. This probably should be removed"
                );
                T::DbWeight::get().reads(1)
            }
        }

        #[cfg(feature = "try-runtime")]
        fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
            Ok(Vec::new())
        }

        #[cfg(feature = "try-runtime")]
        fn post_upgrade(_: Vec<u8>) -> Result<(), &'static str> {
            // new version must be set.
            assert_eq!(Pallet::<T>::on_chain_storage_version(), 2);

            // no reward or bonded pool has been skipped.
            assert_eq!(
                RewardPools::<T>::iter().count() as u32,
                RewardPools::<T>::count()
            );
            assert_eq!(
                BondedPools::<T>::iter().count() as u32,
                BondedPools::<T>::count()
            );
            assert_eq!(
                PoolMembers::<T>::iter().count() as u32,
                PoolMembers::<T>::count()
            );

            log::info!(target: "runtime::nomination-pools", "post upgrade hook for MigrateToV2 executed.");
            Ok(())
        }
    }
}
