use std::ops::Deref;

use anyhow::{bail, Result};
use contract_transcode::Value;
use sp_core::crypto::Ss58Codec;

use crate::AccountId;

/// Temporary wrapper for converting from [Value] to primitive types.
///
/// ```
/// # #![feature(assert_matches)]
/// # #![feature(type_ascription)]
/// # use std::assert_matches::assert_matches;
/// # use anyhow::{anyhow, Result};
/// # use aleph_client::{AccountId, contract::ConvertibleValue};
/// use contract_transcode::Value;
///
/// assert_matches!(ConvertibleValue(Value::UInt(42)).try_into(), Ok(42));
/// assert_matches!(ConvertibleValue(Value::Bool(true)).try_into(), Ok(true));
/// assert_matches!(
///     ConvertibleValue(Value::Literal("5H8cjBBzCJrAvDn9LHZpzzJi2UKvEGC9VeVYzWX5TrwRyVCA".to_string())).
///         try_into(): Result<AccountId>,
///     Ok(_)
/// );
/// assert_matches!(
///     ConvertibleValue(Value::String("not a number".to_string())).try_into(): Result<u128>,
///     Err(_)
/// );
/// ```
#[derive(Debug, Clone)]
pub struct ConvertibleValue(pub Value);

impl Deref for ConvertibleValue {
    type Target = Value;

    fn deref(&self) -> &Value {
        &self.0
    }
}

impl TryFrom<ConvertibleValue> for bool {
    type Error = anyhow::Error;

    fn try_from(value: ConvertibleValue) -> Result<bool, Self::Error> {
        match value.0 {
            Value::Bool(value) => Ok(value),
            _ => bail!("Expected {:?} to be a boolean", value.0),
        }
    }
}

impl TryFrom<ConvertibleValue> for u128 {
    type Error = anyhow::Error;

    fn try_from(value: ConvertibleValue) -> Result<u128, Self::Error> {
        match value.0 {
            Value::UInt(value) => Ok(value),
            _ => bail!("Expected {:?} to be an integer", value.0),
        }
    }
}

impl TryFrom<ConvertibleValue> for AccountId {
    type Error = anyhow::Error;

    fn try_from(value: ConvertibleValue) -> Result<AccountId, Self::Error> {
        match value.0 {
            Value::Literal(value) => Ok(AccountId::from_ss58check(&value)?),
            _ => bail!("Expected {:?} to be a string", value),
        }
    }
}
