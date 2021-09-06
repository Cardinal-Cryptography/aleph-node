use crate::chain_spec::{self, get_account_id_from_seed, AuthorityKeys, LOCAL_AUTHORITIES};
use crate::cli::{Cli, ExtraParams};
use aleph_primitives::AuthorityId as AlephId;
use log::info;
use sc_cli::{CliConfiguration, Error, KeystoreParams, SharedParams};
use sc_keystore::LocalKeystore;
use sc_service::config::{BasePath, KeystoreConfig};
use sp_application_crypto::key_types;
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_core::crypto::KeyTypeId;
use sp_core::{ed25519, sr25519};
use sp_keystore::SyncCryptoStore;
use std::io::Write;
use std::sync::Arc;
use structopt::StructOpt;

/// The `bootstrap-chain` command is used generate keys for the genesis authorities
/// keys are written to the keystore of the authorities
/// and the chain specification is printed to stdout in the JSON format
#[derive(Debug, StructOpt)]
pub struct BootstrapChainCmd {
    /// Force raw genesis storage output.
    #[structopt(long = "raw")]
    pub raw: bool,

    #[structopt(flatten)]
    pub keystore_params: KeystoreParams,

    #[structopt(flatten)]
    pub shared_params: SharedParams,
}

impl CliConfiguration for BootstrapChainCmd {
    fn shared_params(&self) -> &SharedParams {
        &self.shared_params
    }
}

impl BootstrapChainCmd {
    fn upsert_aura_key(
        &self,
        keystore: &LocalKeystore,
        key_type: KeyTypeId,
    ) -> Result<AuraId, Error> {
        let key = SyncCryptoStore::sr25519_public_keys(&*keystore, key_type)
            .into_iter()
            .next()
            .map_or_else(
                || {
                    SyncCryptoStore::sr25519_generate_new(&*keystore, key_type, None)
                        .map_err(|_| Error::KeyStoreOperation)
                },
                Ok,
            );

        match key {
            Ok(k) => {
                let bytes = k.as_array_ref();
                Ok(AuraId::from(sr25519::Public::from_raw(*bytes)))
            }
            Err(why) => panic!("{}", why),
        }
    }

    fn upsert_aleph_key(
        &self,
        keystore: &LocalKeystore,
        key_type: KeyTypeId,
    ) -> Result<AlephId, Error> {
        let key = SyncCryptoStore::ed25519_public_keys(&*keystore, key_type)
            .into_iter()
            .next()
            .map_or_else(
                || {
                    SyncCryptoStore::ed25519_generate_new(&*keystore, key_type, None)
                        .map_err(|_| Error::KeyStoreOperation)
                },
                Ok,
            );

        match key {
            Ok(k) => {
                let bytes = k.as_array_ref();
                Ok(AlephId::from(ed25519::Public::from_raw(*bytes)))
            }
            Err(why) => panic!("{}", why),
        }
    }

    pub fn run(&self, cli: &Cli) -> Result<(), Error> {
        let chain_id = self.shared_params.chain_id(self.shared_params.is_dev());

        let mut genesis_authorities: Vec<AuthorityKeys> = Vec::new();

        LOCAL_AUTHORITIES.iter().for_each(|authority| {
            let authority_keystore = self
                .open_keystore(authority, &chain_id)
                .unwrap_or_else(|_| panic!("Cannot open keystore for {}", authority));

            let aura_key = self
                .upsert_aura_key(&authority_keystore, key_types::AURA)
                .unwrap();
            let aleph_key = self
                .upsert_aleph_key(&authority_keystore, aleph_primitives::KEY_TYPE)
                .unwrap();
            let account_id = get_account_id_from_seed::<sr25519::Public>(authority);

            // NOTE: we could generate libp2p secrets here and add PeerIds to bootstrap nodes list,
            // see Substrate's GenerateNodeKeyCmd

            genesis_authorities.push(AuthorityKeys {
                account_id,
                aura_key,
                aleph_key,
            });
        });

        info!("Building chain spec");

        let ExtraParams {
            session_period,
            millisecs_per_block,
        } = cli.extra;

        let chain_params = chain_spec::ChainParams::from_cli(session_period, millisecs_per_block);
        let chain_spec = match chain_id.as_str() {
            chain_spec::DEVNET_ID => {
                chain_spec::development_config(chain_params, genesis_authorities)
            }
            _ => chain_spec::config(chain_params, genesis_authorities, &chain_id),
        };

        match chain_spec {
            Ok(spec) => {
                let json = sc_service::chain_ops::build_spec(&spec, self.raw)?;
                if std::io::stdout().write_all(json.as_bytes()).is_err() {
                    let _ = std::io::stderr().write_all(b"Error writing to stdout\n");
                }
            }
            Err(why) => panic!("{}", why),
        }

        Ok(())
    }

    fn open_keystore(&self, authority: &str, chain_id: &str) -> Result<Arc<LocalKeystore>, Error> {
        let base_path: BasePath = self
            .shared_params
            .base_path()
            .unwrap()
            .path()
            .join(authority)
            .into();

        info!(
            "Writing to keystore for authority {} and chain id {} under path {:?}",
            authority, chain_id, base_path
        );

        let config_dir = base_path.config_dir(chain_id);
        match self.keystore_params.keystore_config(&config_dir)? {
            (_, KeystoreConfig::Path { path, password }) => {
                Ok(Arc::new(LocalKeystore::open(path, password)?))
            }
            _ => unreachable!("keystore_config always returns path and password; qed"),
        }
    }
}
