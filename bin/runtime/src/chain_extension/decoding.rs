use codec::{Decode, MaxEncodedLen};
use pallet_contracts::chain_extension::{BufInBufOutState, Environment, Ext, SysConfig};
use pallet_snarcos::VerificationKeyIdentifier;
use sp_core::crypto::UncheckedFrom;
use sp_runtime::DispatchError;
use sp_std::{mem::size_of, vec::Vec};

pub type ByteCount = u32;

pub enum DecodingError {
    LimitTooStrict,
    LimitExhausted,
    DecodingFailure(DispatchError),
}

pub trait Reader {
    fn read(&mut self, byte_limit: ByteCount) -> Result<Vec<u8>, DecodingError>;
    fn read_as<T: Decode + MaxEncodedLen>(&mut self) -> Result<T, DecodingError>;
}

impl<E: Ext> Reader for Environment<'_, '_, E, BufInBufOutState>
where
    <E::T as SysConfig>::AccountId: UncheckedFrom<<E::T as SysConfig>::Hash> + AsRef<[u8]>,
{
    fn read(&mut self, byte_limit: ByteCount) -> Result<Vec<u8>, DecodingError> {
        <Environment<'_, '_, E, BufInBufOutState>>::read(self, byte_limit)
            .map_err(DecodingError::DecodingFailure)
    }

    fn read_as<T: Decode + MaxEncodedLen>(&mut self) -> Result<T, DecodingError> {
        <Environment<'_, '_, E, BufInBufOutState>>::read_as::<T>(self)
            .map_err(DecodingError::DecodingFailure)
    }
}

pub trait Decodable: Sized {
    fn decode<R: Reader>(
        reader: &mut R,
        byte_limit: Option<ByteCount>,
    ) -> Result<(Self, ByteCount), DecodingError>;
}

pub struct StoreKeyArgs {
    pub identifier: VerificationKeyIdentifier,
    pub key: Vec<u8>,
}

impl Decodable for StoreKeyArgs {
    fn decode<R: Reader>(
        reader: &mut R,
        byte_limit: Option<ByteCount>,
    ) -> Result<(Self, ByteCount), DecodingError> {
        // We need to read at least a key and a byte count.
        let limit_lower_bound = size_of::<VerificationKeyIdentifier>() + size_of::<ByteCount>();
        if matches!(byte_limit, Some(limit) if limit < limit_lower_bound as ByteCount) {
            return Err(DecodingError::LimitTooStrict);
        }

        let identifier = reader.read_as::<VerificationKeyIdentifier>()?;
        let byte_count = reader.read_as::<ByteCount>()?;

        if matches!(byte_limit, Some(limit) if limit < byte_count) {
            return Err(DecodingError::LimitExhausted);
        }

        let key = reader.read(byte_count)?;

        Ok((StoreKeyArgs { identifier, key }, byte_count))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl Reader for Vec<u8> {
        fn read(&mut self, byte_limit: ByteCount) -> Result<Vec<u8>, DecodingError> {
            Ok(if self.len() <= byte_limit as usize {
                let bytes = self.to_vec();
                *self = Vec::new();
                bytes
            } else {
                let bytes = self[..byte_limit].to_vec();
                *self = self[..byte_limit].to_vec();
                bytes
            })
        }

        fn read_as<T: Decode + MaxEncodedLen>(&mut self) -> Result<T, DecodingError> {
            let mut bytes = self.read(<T as MaxEncodedLen>::max_encoded_len() as ByteCount)?;
            <T as Decode>::decode(&mut bytes).map_err(DecodingError::DecodingFailure)
        }
    }

    #[test]
    fn store_keys_limit_must_allow_to_read_necessary_data() {}
}
