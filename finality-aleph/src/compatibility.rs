use codec::Encode;

pub type Version = u16;
pub type ByteCount = u16;

pub fn encode_with_version(version: Version, mut payload: Vec<u8>) -> Vec<u8> {
    let mut result = version.encode();
    // This will produce rubbish if we ever try encodings that have more than u16::MAX bytes. We
    // expect this won't happen, since we will switch to proper multisignatures before proofs get
    // that big.
    let num_bytes = payload.len() as ByteCount;
    result.append(&mut num_bytes.encode());
    result.append(&mut payload);
    result
}
