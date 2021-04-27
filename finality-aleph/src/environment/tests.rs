use super::*;

#[derive(Debug, PartialEq, Clone, Eq, serde::Serialize)]
pub struct Extrinsic {}

parity_util_mem::malloc_size_of_is_0!(Extrinsic);

impl ExtrinsicT for Extrinsic {
    type Call = Extrinsic;
    type SignaturePayload = ();
}

pub type BlockNumber = u64;

pub type Hashing = sp_runtime::traits::BlakeTwo256;

pub type Header = sp_runtime::generic::Header<BlockNumber, Hashing>;

pub type Hash = H256;

pub type Block = sp_runtime::generic::Block<Header, Extrinsic>;

type Backend = sc_client_api::in_mem::Backend<Block>;

struct Client { }

impl sc_client_api::LockImportRun<Block, Backend> for Client {
    fn lock_import_and_run<R, Err, F>(&self, f: F) -> Result<R, Err>, where
        F: FnOnce(&mut ClientImportOperation<Block, B>) -> Result<R, Err>,
        Err: From<sp_blockchain::Error> {
        todo!()
    }
}

impl sc_client_api::Finalizer<Block, Backend> for Client {
    fn apply_finality(&self, operation: &mut ClientImportOperation<Block, Backend>, id: BlockId<Block>, justification: Option<Justification>, notify: bool) -> _ {
        todo!()
    }

    fn finalize_block(&self, id: BlockId<Block>, justification: Option<Justification>, notify: bool) -> Result<()> {
        todo!()
    }
}

impl sp_api::ProvideRuntimeApi<Block> for Client {

}

impl HeaderBackend<Block> for Client {

}

impl HeaderMetadata<Block> for Client {

}


#[test]
fn test() {
}