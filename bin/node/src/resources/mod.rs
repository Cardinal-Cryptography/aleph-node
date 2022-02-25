pub(crate) fn mainnet_chainspec() -> &'static [u8] {
    include_bytes!("mainnet_chainspec.json")
}

pub(crate) fn testnet_chainspec() -> &'static [u8] {
    include_bytes!("testnet_chainspec.json")
}
