use std::{
    io::Error as IoError,
    iter,
    net::{SocketAddr, ToSocketAddrs as _},
};

use derive_more::{AsRef, Display};
use log::info;
use network_clique::{Dialer, Listener, PeerId, PublicKey, SecretKey};
use parity_scale_codec::{Decode, Encode};
use sp_core::crypto::KeyTypeId;
use tokio::net::{TcpListener, TcpStream, ToSocketAddrs};

use crate::{
    aleph_primitives::AuthorityId,
    crypto::{verify, AuthorityPen, Signature},
    network::{AddressingInformation, NetworkIdentity},
};

const LOG_TARGET: &str = "tcp-network";

pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"a0vn");

#[derive(PartialEq, Eq, Clone, Debug, Display, Hash, Decode, Encode, AsRef)]
#[as_ref(forward)]
pub struct AuthorityIdWrapper(AuthorityId);

impl From<AuthorityId> for AuthorityIdWrapper {
    fn from(value: AuthorityId) -> Self {
        AuthorityIdWrapper(value)
    }
}

impl PeerId for AuthorityIdWrapper {}

impl PublicKey for AuthorityIdWrapper {
    type Signature = Signature;

    fn verify(&self, message: &[u8], signature: &Self::Signature) -> bool {
        verify(&self.0, message, signature)
    }
}

#[async_trait::async_trait]
impl SecretKey for AuthorityPen {
    type Signature = Signature;
    type PublicKey = AuthorityIdWrapper;

    fn sign(&self, message: &[u8]) -> Self::Signature {
        AuthorityPen::sign(self, message)
    }

    fn public_key(&self) -> Self::PublicKey {
        self.authority_id().into()
    }
}

/// What can go wrong when handling addressing information.
#[derive(Debug, Hash, Clone, PartialEq, Eq)]
pub enum AddressingInformationError {
    /// Construction of an addressing information object requires at least one address.
    NoAddress,
}

#[derive(Debug, Hash, Encode, Decode, Clone, PartialEq, Eq)]
struct TcpAddressingInformation {
    peer_id: AuthorityId,
    // Easiest way to ensure that the Vec below is nonempty...
    primary_address: String,
    other_addresses: Vec<String>,
}

impl TcpAddressingInformation {
    fn new(
        addresses: Vec<String>,
        peer_id: AuthorityId,
    ) -> Result<TcpAddressingInformation, AddressingInformationError> {
        let mut addresses = addresses.into_iter();
        let primary_address = match addresses.next() {
            Some(address) => address,
            None => return Err(AddressingInformationError::NoAddress),
        };
        Ok(TcpAddressingInformation {
            primary_address,
            other_addresses: addresses.collect(),
            peer_id,
        })
    }

    fn peer_id(&self) -> AuthorityId {
        self.peer_id.clone()
    }
}

/// A representation of TCP addressing information with an associated peer ID, self-signed.
#[derive(Debug, Hash, Encode, Decode, Clone, PartialEq, Eq)]
pub struct SignedTcpAddressingInformation {
    addressing_information: TcpAddressingInformation,
    signature: Signature,
}

impl AddressingInformation for SignedTcpAddressingInformation {
    type PeerId = AuthorityIdWrapper;

    fn peer_id(&self) -> Self::PeerId {
        self.addressing_information.peer_id().into()
    }

    fn verify(&self) -> bool {
        self.peer_id()
            .verify(&self.addressing_information.encode(), &self.signature)
    }

    fn lower_level_address(&self) -> String {
        iter::once(self.addressing_information.primary_address.clone())
            .chain(self.addressing_information.other_addresses.clone())
            .filter_map(|address| Some(address.parse::<SocketAddr>().ok()?.to_string()))
            .next()
            .unwrap_or("unknown".to_string())
    }
}

impl NetworkIdentity for SignedTcpAddressingInformation {
    type PeerId = AuthorityIdWrapper;
    type AddressingInformation = SignedTcpAddressingInformation;

    fn identity(&self) -> Self::AddressingInformation {
        self.clone()
    }
}

impl SignedTcpAddressingInformation {
    fn new(
        addresses: Vec<String>,
        authority_pen: &AuthorityPen,
    ) -> Result<SignedTcpAddressingInformation, AddressingInformationError> {
        let peer_id = authority_pen.authority_id();
        let addressing_information = TcpAddressingInformation::new(addresses, peer_id)?;
        let signature = authority_pen.sign(&addressing_information.encode());
        Ok(SignedTcpAddressingInformation {
            addressing_information,
            signature,
        })
    }
}

#[derive(Clone)]
struct TcpDialer;

#[async_trait::async_trait]
impl Dialer<SignedTcpAddressingInformation> for TcpDialer {
    type Connection = TcpStream;
    type Error = std::io::Error;

    async fn connect(
        &mut self,
        address: SignedTcpAddressingInformation,
    ) -> Result<Self::Connection, Self::Error> {
        let SignedTcpAddressingInformation {
            addressing_information,
            ..
        } = address;
        let TcpAddressingInformation {
            primary_address,
            other_addresses,
            ..
        } = addressing_information;
        let parsed_addresses: Vec<_> = iter::once(primary_address)
            .chain(other_addresses)
            .filter_map(|address| address.to_socket_addrs().ok())
            .flatten()
            .collect();
        let stream = TcpStream::connect(&parsed_addresses[..]).await?;
        if stream.set_linger(None).is_err() {
            info!(target: LOG_TARGET, "stream.set_linger(None) failed.");
        };
        Ok(stream)
    }
}

/// Possible errors when creating a TCP network.
#[derive(Debug)]
pub enum Error {
    Io(IoError),
    AddressingInformation(AddressingInformationError),
}

impl From<IoError> for Error {
    fn from(e: IoError) -> Self {
        Error::Io(e)
    }
}

impl From<AddressingInformationError> for Error {
    fn from(e: AddressingInformationError) -> Self {
        Error::AddressingInformation(e)
    }
}

/// Create a new tcp network, including an identity that can be used for constructing
/// authentications for other peers.
pub async fn new_tcp_network<A: ToSocketAddrs>(
    listening_addresses: A,
    external_addresses: Vec<String>,
    authority_pen: &AuthorityPen,
) -> Result<
    (
        impl Dialer<SignedTcpAddressingInformation>,
        impl Listener,
        impl NetworkIdentity<
            AddressingInformation = SignedTcpAddressingInformation,
            PeerId = AuthorityIdWrapper,
        >,
    ),
    Error,
> {
    let listener = TcpListener::bind(listening_addresses).await?;
    let identity = SignedTcpAddressingInformation::new(external_addresses, authority_pen)?;
    Ok((TcpDialer {}, listener, identity))
}

#[cfg(test)]
pub mod testing {
    use network_clique::AddressingInformation;

    use super::{AuthorityIdWrapper, SignedTcpAddressingInformation};
    use crate::{
        crypto::AuthorityPen,
        network::{mock::crypto_basics, NetworkIdentity},
    };

    /// Creates a realistic identity.
    pub fn new_identity(
        external_addresses: Vec<String>,
        authority_pen: &AuthorityPen,
    ) -> impl NetworkIdentity<
        AddressingInformation = SignedTcpAddressingInformation,
        PeerId = AuthorityIdWrapper,
    > {
        SignedTcpAddressingInformation::new(external_addresses, authority_pen)
            .expect("the provided addresses are fine")
    }

    #[test]
    pub fn lower_level_address_parses_ip() {
        let (_, authority_pen) = &crypto_basics(1).0[0];
        for ip in ["127.0.0.1", "[::1]", "[2607:f8b0:4003:c00::6a]"] {
            let ip_and_port = format!("{}:1234", ip);
            let addr =
                SignedTcpAddressingInformation::new(vec![ip_and_port.clone()], authority_pen)
                    .unwrap();

            assert_eq!(addr.lower_level_address(), ip_and_port.to_string())
        }
    }
    #[test]
    pub fn lower_level_address_doesnt_try_to_resolve_dns() {
        let (_, authority_pen) = &crypto_basics(1).0[0];
        let addr = SignedTcpAddressingInformation::new(
            vec!["bootnode-eu-west-1-0.test.azero.dev:30333".to_string()],
            authority_pen,
        )
        .unwrap();
        assert_eq!(addr.lower_level_address(), "unknown".to_string())
    }
}
