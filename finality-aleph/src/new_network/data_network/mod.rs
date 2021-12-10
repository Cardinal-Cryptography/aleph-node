use crate::Encode;
use crate::Decode;
use crate::NodeIndex;
use codec::Codec;
use crate::SessionId;
use crate::new_network::PeerId;
use crate::new_network::connection_manager::AuthData;
use crate::crypto::Signature;

mod aleph_network;
mod data_network;
mod rmc_network;

#[derive(Clone, Encode, Decode, Debug)]
pub(crate) enum MetaMessage {
    Authentication(AuthData, Signature),
    AuthenticationRequest(SessionId),
}

#[derive(Clone, Encode, Decode, Debug)]
pub(crate) enum ControlCommand {
    Terminate(SessionId),
}

#[derive(Clone, Encode, Decode)]
enum SessionCommand<D: Clone + Encode + Decode> {
    Meta(MetaMessage, Recipient<PeerId>),
    Data(SessionId, D, Recipient<NodeIndex>),
    Control(ControlCommand),
}

impl<D: Clone + Codec> SessionCommand<D> {
    fn map<E: Clone + Codec, F: FnOnce(D) -> E>(self, f: F) -> SessionCommand<E> {
        use SessionCommand::*;
        match self {
            Meta(message, recipient) => Meta(message, recipient),
            Data(session_id, data, recipient) => Data(session_id, f(data), recipient),
            Control(cc) => Control(cc),
        }
    }
}

#[derive(Clone, Copy, Encode, Decode, Debug, Eq, PartialEq)]
pub(crate) enum Recipient<T: Clone + Encode + Decode + Eq + PartialEq> {
    All,
    Target(T),
}

impl From<aleph_bft::Recipient> for Recipient<NodeIndex> {
    fn from(recipient: aleph_bft::Recipient) -> Self {
        match recipient {
            aleph_bft::Recipient::Everyone => Recipient::All,
            aleph_bft::Recipient::Node(node) => Recipient::Target(node),
        }
    }
}