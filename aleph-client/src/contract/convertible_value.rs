use std::{ops::Deref, str::FromStr};

use anyhow::{anyhow, bail, Result};
use contract_transcode::Value;

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
/// assert_matches!(ConvertibleValue(Value::UInt(42)).try_into(), Ok(42u128));
/// assert_matches!(ConvertibleValue(Value::UInt(42)).try_into(), Ok(42u32));
/// assert_matches!(ConvertibleValue(Value::UInt(u128::MAX)).try_into(): Result<u32>, Err(_));
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

impl TryFrom<ConvertibleValue> for u32 {
    type Error = anyhow::Error;

    fn try_from(value: ConvertibleValue) -> Result<u32, Self::Error> {
        match value.0 {
            Value::UInt(value) => Ok(value.try_into()?),
            _ => bail!("Expected {:?} to be an integer", value.0),
        }
    }
}

impl TryFrom<ConvertibleValue> for AccountId {
    type Error = anyhow::Error;

    fn try_from(value: ConvertibleValue) -> Result<AccountId, Self::Error> {
        match value.0 {
            Value::Literal(value) => {
                AccountId::from_str(&value).map_err(|_| anyhow!("Invalid account id"))
            }
            _ => bail!("Expected {:?} to be a string", value),
        }
    }
}

impl<T> TryFrom<ConvertibleValue> for Result<T>
where
    ConvertibleValue: TryInto<T, Error = anyhow::Error>,
{
    type Error = anyhow::Error;

    fn try_from(value: ConvertibleValue) -> Result<Result<T>, Self::Error> {
        if let Value::Tuple(tuple) = &value.0 {
            match tuple.ident() {
                Some(x) if x == "Ok" => {
                    if tuple.values().count() == 1 {
                        let item =
                            ConvertibleValue(tuple.values().next().unwrap().clone()).try_into()?;
                        return Ok(Ok(item));
                    } else {
                        bail!("Unexpected number of elements in Ok variant: {:?}", &value);
                    }
                }
                Some(x) if x == "Err" => {
                    if tuple.values().count() == 1 {
                        return Ok(Err(anyhow!(value.to_string())));
                    } else {
                        bail!("Unexpected number of elements in Err variant: {:?}", &value);
                    }
                }
                _ => (),
            }
        }

        bail!("Expected {:?} to be an Ok(_) or Err(_) tuple.", value);
    }
}
