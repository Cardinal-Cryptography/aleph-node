use ink::env::Environment;
use scale::{Decode, Encode};

#[derive(Debug, Encode, Decode, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum Role<E: Environment> {
    /// Indicates a superuser.
    Admin(E::AccountId),
    /// Indicates account that can terminate a contract.
    Owner(E::AccountId),
    /// Indicates account that can initialize a contract from a given code hash.
    Initializer(E::Hash),
    /// Indicates account that can add liquidity to a DEX contract (call certain functions)
    LiquidityProvider(E::AccountId),
    /// Indicates account that can mint tokens of a given token contract,
    Minter(E::AccountId),
    /// Indicates account that can burn tokens of a given token contract,
    Burner(E::AccountId),
}
