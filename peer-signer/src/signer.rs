use clap::Parser;
use libp2p::identity::{ed25519 as libp2p_ed25519, PublicKey};
use std::fs;

#[derive(Debug, Parser)]
#[clap(version = "1.0")]
pub struct Config {
    /// Message we want to sign.
    #[clap(long)]
    pub message: String,

    /// Path to p2p secret.
    #[clap(long)]
    pub p2p_secret: String,
}

fn main() {
    let Config {
        message,
        p2p_secret: p2p_secret_path,
    } = Config::parse();

    let mut file_content = fs::read(&p2p_secret_path).expect("Can not read from p2p secret path");
    let secret_key = libp2p_ed25519::SecretKey::from_bytes(&mut file_content)
        .expect("Incorrect secret. Can't create a secret key.");

    let keypair = libp2p_ed25519::Keypair::from(secret_key);
    let public = PublicKey::Ed25519(keypair.public());

    println!(
        "Public key: {}",
        hex::encode(&public.to_protobuf_encoding())
    );
    println!(
        "Signed message: {}",
        hex::encode(&keypair.sign(message.as_bytes()))
    );
}
