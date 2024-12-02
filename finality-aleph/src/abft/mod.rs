//! Main purpose of this module is to be able to use two different versions of the abft crate.
//! Older version is referred to as 'Legacy' while newer as 'Current'.
//! We achieve this by hiding types & traits from abft crates behind our owns. In case of traits we
//! implement both current and legacy ones. In case of types we implement trait `From` to be able
//! convert them at the 'glueing' spot to the abft library. Current and legacy versions are marked
//! by numbers. Whenever we upgrade to next version of abft we need to increment and mark each version
//! version accordingly.

mod common;
mod crypto;
mod current;
mod legacy;
mod network;
mod traits;
mod types;

pub use crypto::Keychain;
pub use current::{
    create_aleph_config as current_create_aleph_config, run_member as run_current_member,
    NetworkData as CurrentNetworkData, VERSION as CURRENT_VERSION,
};
pub use legacy::{
    create_aleph_config as legacy_create_aleph_config, run_member as run_legacy_member,
    NetworkData as LegacyNetworkData, VERSION as LEGACY_VERSION,
};
pub use network::NetworkWrapper;
pub use traits::{SpawnHandle, Wrapper as HashWrapper};
pub use types::{NodeCount, NodeIndex, Recipient};

pub use primitives::SignatureSet;
