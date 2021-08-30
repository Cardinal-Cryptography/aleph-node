use sc_cli::{Error, KeystoreParams, RunCmd, SharedParams};
use sc_service::config::{BasePath, KeystoreConfig};
use std::sync::Arc;

use aleph_node::chain_spec::{get_account_id_from_seed, AuthorityKeys};
use sc_keystore::LocalKeystore;
use sp_keystore::{SyncCryptoStore, SyncCryptoStorePtr};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct Cli {
    #[structopt(subcommand)]
    pub subcommand: Option<Subcommand>,

    #[structopt(flatten)]
    pub run: RunCmd,

    #[structopt(flatten)]
    pub extra: ExtraParams,
}

#[derive(Clone, Debug, Default, StructOpt)]
pub struct ExtraParams {
    #[structopt(long)]
    pub(crate) session_period: Option<u32>,

    #[structopt(long)]
    pub(crate) millisecs_per_block: Option<u64>,

    #[structopt(long)]
    pub keys_path: Option<String>,
}

#[derive(Debug, StructOpt)]
pub struct GenerateKeysCmd {
    /// List of genesis authorities
    #[structopt(long)]
    pub authorities: Vec<String>,

    /// Path to write json with aleph and aura keys
    #[structopt(long)]
    pub keys_path: Option<String>,

    #[structopt(flatten)]
    pub keystore_params: KeystoreParams,

    #[structopt(flatten)]
    pub shared_params: SharedParams,
}

impl GenerateKeysCmd {
    pub fn run(&self) -> Result<(), Error> {
        let authority_keys: Vec<AuthorityKeys> = crate::chain_spec::LOCAL_AUTHORITIES
            .iter()
            .map(|authority| self.authority_keys(*authority))
            .collect::<Result<Vec<_>, Error>>()?;

        let auth_keys = serde_json::to_string(&authority_keys).map_err(|e| Error::Io(e.into()))?;
        std::fs::write(self.keys_path.as_ref().unwrap().as_str(), &auth_keys).map_err(Error::Io)?;

        Ok(())
    }

    fn authority_keys(&self, authority: &str) -> Result<AuthorityKeys, Error> {
        let account_id = get_account_id_from_seed::<sp_core::sr25519::Public>(authority);

        let keystore = self.open_keystore(authority)?;
        let aura_key = match SyncCryptoStore::sr25519_public_keys(
            &*keystore,
            sp_core::crypto::key_types::AURA,
        )
        .pop()
        {
            Some(key) => key,
            None => SyncCryptoStore::sr25519_generate_new(
                &*keystore,
                sp_core::crypto::key_types::AURA,
                None,
            )
            .map_err(|_| Error::KeyStoreOperation)?,
        }
        .into();

        let aleph_key = match SyncCryptoStore::ed25519_public_keys(
            &*keystore,
            aleph_primitives::KEY_TYPE,
        )
        .pop()
        {
            Some(key) => key,
            None => {
                SyncCryptoStore::ed25519_generate_new(&*keystore, aleph_primitives::KEY_TYPE, None)
                    .map_err(|_| Error::KeyStoreOperation)?
            }
        }
        .into();

        Ok(AuthorityKeys {
            account_id,
            aura_key,
            aleph_key,
        })
    }

    fn open_keystore(&self, authority: &str) -> Result<SyncCryptoStorePtr, Error> {
        let base_path: BasePath = self
            .shared_params
            .base_path()
            .unwrap()
            .path()
            .join(authority)
            .into();
        let chain_id = self.shared_params.chain_id(self.shared_params.is_dev());
        let config_dir = base_path.config_dir(&chain_id);

        match self.keystore_params.keystore_config(&config_dir)? {
            (_, KeystoreConfig::Path { path, password }) => {
                Ok(Arc::new(LocalKeystore::open(path, password)?))
            }
            _ => unreachable!("keystore_config always returns path and password; qed"),
        }
    }
}

#[derive(Debug, StructOpt)]
pub enum Subcommand {
    /// Key management cli utilities
    Key(sc_cli::KeySubcommand),
    /// Build a chain specification.
    BuildSpec(sc_cli::BuildSpecCmd),

    /// Validate blocks.
    CheckBlock(sc_cli::CheckBlockCmd),

    /// Export blocks.
    ExportBlocks(sc_cli::ExportBlocksCmd),

    /// Export the state of a given block into a chain spec.
    ExportState(sc_cli::ExportStateCmd),

    /// Import blocks.
    ImportBlocks(sc_cli::ImportBlocksCmd),

    /// Remove the whole chain.
    PurgeChain(sc_cli::PurgeChainCmd),

    /// Revert the chain to a previous state.
    Revert(sc_cli::RevertCmd),

    /// Generate keys for local tests
    DevKeys(GenerateKeysCmd),
}
