use tendermint::{
    block::{self, header::Version, parts::Header as PartSetHeader, Commit, CommitSig, Header},
    chain::{self, Id},
    hash::{self, Hash},
    validator::Info,
};
use tendermint_light_client_verifier::{
    options::Options,
    types::{LightBlock, PeerId, SignedHeader, TrustThreshold, ValidatorSet},
};

pub fn sha256_from_bytes(hash: &[u8]) -> Hash {
    // TODO type enforce 32 bytes
    Hash::from_bytes(hash::Algorithm::Sha256, hash).expect("Can't produce hash from bytes")
}
