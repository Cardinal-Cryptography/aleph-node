#![allow(unused_variables)]
use sc_client_api::{Backend, ClientImportOperation};
use sp_api::{ApiError, Hasher, RuntimeApiInfo, StorageChanges};
use sp_application_crypto::sp_core::storage::ChildInfo;
use sp_runtime::{
    generic::BlockId,
    traits::{Block, HashFor, NumberFor},
    TransactionOutcome,
};
use sp_state_machine::{
    ChangesTrieState, DBValue, DefaultError, ProofRecorder, StateMachineStats, StorageKey,
    StorageValue, UsageInfo,
};
use sp_trie::StorageProof;

#[derive(Clone, Debug)]
pub struct Dummy<T = ()> {
    _phantom: std::marker::PhantomData<T>,
}

impl<T> Default for Dummy<T> {
    fn default() -> Self {
        Dummy {
            _phantom: std::marker::PhantomData,
        }
    }
}

impl std::fmt::Display for Dummy {
    fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> core::fmt::Result {
        panic!()
    }
}

impl sp_state_machine::backend::Consolidate for Dummy {
    fn consolidate(&mut self, _: Self) {
        panic!()
    }
}

impl<H: Hasher> hash_db::AsHashDB<H, DBValue> for Dummy {
    fn as_hash_db(&self) -> &dyn hash_db::HashDB<H, DBValue> {
        panic!()
    }

    fn as_hash_db_mut(&mut self) -> &mut dyn hash_db::HashDB<H, DBValue> {
        panic!()
    }
}

impl<H: Hasher> hash_db::HashDB<H, DBValue> for Dummy {
    fn get(&self, _: &<H as Hasher>::Out, _: (&[u8], Option<u8>)) -> Option<DBValue> {
        panic!()
    }

    fn contains(&self, _: &<H as Hasher>::Out, _: (&[u8], Option<u8>)) -> bool {
        panic!()
    }

    fn insert(&mut self, _: (&[u8], Option<u8>), _: &[u8]) -> <H as Hasher>::Out {
        panic!()
    }

    fn emplace(&mut self, _: <H as Hasher>::Out, _: (&[u8], Option<u8>), _: DBValue) {
        panic!()
    }

    fn remove(&mut self, _: &<H as Hasher>::Out, _: (&[u8], Option<u8>)) {
        panic!()
    }
}

impl<H: Hasher> sp_state_machine::TrieBackendStorage<H> for Dummy {
    type Overlay = Dummy;

    fn get(
        &self,
        _: &<H as Hasher>::Out,
        _: (&[u8], Option<u8>),
    ) -> Result<Option<DBValue>, DefaultError> {
        panic!()
    }
}

impl<H: Hasher> sp_state_machine::Backend<H> for Dummy {
    type Error = Dummy;
    type Transaction = ();
    type TrieBackendStorage = Dummy;

    fn storage(&self, _: &[u8]) -> Result<Option<StorageValue>, Self::Error> {
        panic!()
    }

    fn child_storage(&self, _: &ChildInfo, _: &[u8]) -> Result<Option<StorageValue>, Self::Error> {
        panic!()
    }

    fn next_storage_key(&self, _: &[u8]) -> Result<Option<StorageKey>, Self::Error> {
        panic!()
    }

    fn next_child_storage_key(
        &self,
        _: &ChildInfo,
        _: &[u8],
    ) -> Result<Option<StorageKey>, Self::Error> {
        panic!()
    }

    fn apply_to_child_keys_while<F: FnMut(&[u8]) -> bool>(&self, _: &ChildInfo, _: F) {
        panic!()
    }

    fn for_key_values_with_prefix<F: FnMut(&[u8], &[u8])>(&self, _: &[u8], _: F) {
        panic!()
    }

    fn for_child_keys_with_prefix<F: FnMut(&[u8])>(&self, _: &ChildInfo, _: &[u8], _: F) {
        panic!()
    }

    fn storage_root<'a>(
        &self,
        _: impl Iterator<Item = (&'a [u8], Option<&'a [u8]>)>,
    ) -> (<H as Hasher>::Out, Self::Transaction)
    where
        H::Out: Ord,
    {
        panic!()
    }

    fn child_storage_root<'a>(
        &self,
        _: &ChildInfo,
        _: impl Iterator<Item = (&'a [u8], Option<&'a [u8]>)>,
    ) -> (<H as Hasher>::Out, bool, Self::Transaction)
    where
        H::Out: Ord,
    {
        panic!()
    }

    fn pairs(&self) -> Vec<(StorageKey, StorageValue)> {
        panic!()
    }

    fn register_overlay_stats(&mut self, _: &StateMachineStats) {
        panic!()
    }

    fn usage_info(&self) -> UsageInfo {
        panic!()
    }
}

impl<B: Block> sp_api::ApiExt<B> for Dummy {
    type StateBackend = Dummy;

    fn execute_in_transaction<F: FnOnce(&Self) -> TransactionOutcome<R>, R>(&self, _: F) -> R
    where
        Self: Sized,
    {
        panic!()
    }

    fn has_api<A: RuntimeApiInfo + ?Sized>(&self, _: &BlockId<B>) -> Result<bool, ApiError>
    where
        Self: Sized,
    {
        panic!()
    }

    fn has_api_with<A: RuntimeApiInfo + ?Sized, P: Fn(u32) -> bool>(
        &self,
        _: &BlockId<B>,
        _: P,
    ) -> Result<bool, ApiError>
    where
        Self: Sized,
    {
        panic!()
    }

    fn record_proof(&mut self) {
        panic!()
    }

    fn extract_proof(&mut self) -> Option<StorageProof> {
        panic!()
    }

    fn proof_recorder(&self) -> Option<ProofRecorder<<B as Block>::Hash>> {
        panic!()
    }

    fn into_storage_changes(
        &self,
        _: &Self::StateBackend,
        _: Option<&ChangesTrieState<HashFor<B>, NumberFor<B>>>,
        _: <B as Block>::Hash,
    ) -> Result<StorageChanges<Self::StateBackend, B>, String>
    where
        Self: Sized,
    {
        panic!()
    }
}

impl<B: Block, BE: sc_client_api::Backend<B>> sc_client_api::LockImportRun<B, BE> for Dummy<BE> {
    fn lock_import_and_run<R, Err, F>(&self, _f: F) -> Result<R, Err>
    where
        F: FnOnce(&mut ClientImportOperation<B, BE>) -> Result<R, Err>,
        Err: From<sp_blockchain::Error>,
    {
        panic!()
    }
}

impl<B: Block, BE: Backend<B>> sc_client_api::Finalizer<B, BE> for Dummy<BE> {
    fn apply_finality(
        &self,
        operation: &mut ClientImportOperation<B, BE>,
        id: BlockId<B>,
        justification: Option<sp_runtime::Justification>,
        notify: bool,
    ) -> sp_blockchain::Result<()> {
        panic!()
    }

    fn finalize_block(
        &self,
        id: BlockId<B>,
        justification: Option<sp_runtime::Justification>,
        notify: bool,
    ) -> sp_blockchain::Result<()> {
        panic!()
    }
}

impl<B: sp_runtime::traits::Block, BE: sc_client_api::Backend<B>> sp_api::ProvideRuntimeApi<B>
    for Dummy<BE>
{
    type Api = Dummy;

    fn runtime_api(&self) -> sp_api::ApiRef<Self::Api> {
        panic!()
    }
}

impl<B: sp_runtime::traits::Block, BE: sc_client_api::Backend<B>> sp_blockchain::HeaderBackend<B>
    for Dummy<BE>
{
    fn header(&self, _: BlockId<B>) -> sp_blockchain::Result<Option<B::Header>> {
        panic!()
    }

    fn info(&self) -> sp_blockchain::Info<B> {
        panic!()
    }

    fn status(&self, _: BlockId<B>) -> sp_blockchain::Result<sp_blockchain::BlockStatus> {
        panic!()
    }

    fn number(
        &self,
        _: B::Hash,
    ) -> sp_blockchain::Result<Option<<B::Header as sp_runtime::traits::Header>::Number>> {
        panic!()
    }

    fn hash(
        &self,
        _: <B::Header as sp_runtime::traits::Header>::Number,
    ) -> sp_blockchain::Result<Option<B::Hash>> {
        panic!()
    }
}

impl<B: Block, BE: sc_client_api::Backend<B>> sp_blockchain::HeaderMetadata<B> for Dummy<BE> {
    type Error = sp_blockchain::Error;

    fn header_metadata(
        &self,
        hash: B::Hash,
    ) -> Result<sp_blockchain::CachedHeaderMetadata<B>, Self::Error> {
        panic!()
    }

    fn insert_header_metadata(
        &self,
        hash: B::Hash,
        header_metadata: sp_blockchain::CachedHeaderMetadata<B>,
    ) {
        panic!()
    }

    fn remove_header_metadata(&self, hash: B::Hash) {
        panic!()
    }
}

#[async_trait::async_trait]
impl<B: Block, BE: Backend<B>> sp_consensus::BlockImport<B> for Dummy<BE>
where
    sc_client_api::TransactionFor<BE, B>: 'static,
{
    type Error = sp_consensus::Error;
    type Transaction = sc_client_api::TransactionFor<BE, B>;

    async fn check_block(
        &mut self,
        block: sp_consensus::BlockCheckParams<B>,
    ) -> Result<sp_consensus::ImportResult, Self::Error> {
        panic!()
    }

    async fn import_block(
        &mut self,
        block: sp_consensus::BlockImportParams<B, Self::Transaction>,
        cache: std::collections::HashMap<[u8; 4], Vec<u8>>,
    ) -> Result<sp_consensus::ImportResult, Self::Error> {
        panic!()
    }
}
