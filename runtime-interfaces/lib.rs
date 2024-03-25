//! A collection of runtime interfaces (Substrate's concept for outsourcing computation to the host) for Aleph Zero
//! chain.

#![cfg_attr(not(feature = "std"), no_std)]

#[sp_runtime_interface::runtime_interface]
pub trait Now {
    fn now() -> Result<i64, ()> {
        #[cfg(not(feature = "std"))]
        unreachable!();

        #[cfg(feature = "std")]
        Ok(chrono::prelude::Utc::now().timestamp_nanos_opt().unwrap())
    }
}
