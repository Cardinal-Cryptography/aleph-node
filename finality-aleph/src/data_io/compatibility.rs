use std::mem::size_of;

use codec::{Decode, Encode, Error as CodecError, Input as CodecInput};

use super::AlephData;
use crate::{
    compatibility::{encode_with_version, ByteCount, Version},
    data_io::BlockT,
};

#[derive(Clone, Debug)]
pub enum VersionedAlephData<B: BlockT> {
    Other(Version, Vec<u8>),
    V1(AlephData<B>),
}

impl<B: BlockT> Encode for VersionedAlephData<B> {
    fn size_hint(&self) -> usize {
        use VersionedAlephData::*;
        let version_size = size_of::<Version>();
        let byte_count_size = size_of::<ByteCount>();
        version_size
            + byte_count_size
            + match self {
                Other(_, payload) => payload.len(),
                V1(justification) => justification.size_hint(),
            }
    }

    fn encode(&self) -> Vec<u8> {
        use VersionedAlephData::*;
        match self {
            Other(version, payload) => encode_with_version(*version, payload.clone()),
            V1(justification) => encode_with_version(1, justification.encode()),
        }
    }
}

impl<B: BlockT> Decode for VersionedAlephData<B> {
    fn decode<I: CodecInput>(input: &mut I) -> Result<Self, CodecError> {
        use VersionedAlephData::*;
        let version = Version::decode(input)?;
        let num_bytes = ByteCount::decode(input)?;
        match version {
            1 => Ok(V1(AlephData::decode(input)?)),
            _ => {
                let mut payload = vec![0; num_bytes.into()];
                input.read(payload.as_mut_slice())?;
                Ok(Other(version, payload))
            }
        }
    }
}

#[cfg(test)]
mod test {
    use codec::{Decode, Encode};

    use crate::{
        data_io::{AlephData, UnvalidatedAlephProposal},
        testing::mocks::{TBlock, THash, TNumber},
    };

    #[test]
    fn correctly_decodes_v1() {
        let branch = vec![THash::default(); 1];
        let number = TNumber::default();
        let data_v1: AlephData<TBlock> =
            AlephData { head_proposal: UnvalidatedAlephProposal::new(branch, number) };
        let decoded = AlephData::decode(&mut data_v1.encode().as_slice());
        assert_eq!(decoded, Ok(data_v1));
    }
}
