/// Decode hex string with or without 0x prefix
pub fn decode_hex(input: &str) -> Result<Vec<u8>, hex::FromHexError> {
    if input.starts_with("0x") {
        hex::decode(input.trim_start_matches("0x"))
    } else {
        hex::decode(input)
    }
}
