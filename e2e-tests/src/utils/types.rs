use codec::Compact;
use sp_core::sr25519;
use sp_runtime::{generic, traits::BlakeTwo256, MultiAddress};
use substrate_api_client::rpc::WsRpcClient;
use substrate_api_client::{AccountId, Api, UncheckedExtrinsicV4};

pub type BlockNumber = u32;
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
pub type KeyPair = sr25519::Pair;
pub type Connection = Api<KeyPair, WsRpcClient>;
pub type TransferTransaction = UncheckedExtrinsicV4<([u8; 2], MultiAddress<AccountId, ()>, Compact<u128>)>;
