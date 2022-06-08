use crate::{
    crypto::KeyBox,
    data_io::{AlephData, OrderedDataInterpreter},
    network::{AlephNetworkData, DataNetwork, NetworkWrapper},
    party::{AuthoritySubtaskCommon, Task},
};
use aleph_bft::{Config, LocalIO, SpawnHandle};
use futures::channel::oneshot;
use log::debug;
use sc_client_api::HeaderBackend;
use sp_runtime::traits::Block;
use std::{
    fs,
    fs::File,
    io,
    io::{Read, Write},
    path::{Path, PathBuf},
    str::FromStr,
};

/// Runs the member within a single session.
pub fn task<
    B: Block,
    C: HeaderBackend<B> + Send + 'static,
    ADN: DataNetwork<AlephNetworkData<B>> + 'static,
>(
    subtask_common: AuthoritySubtaskCommon,
    multikeychain: KeyBox,
    config: Config,
    network: NetworkWrapper<AlephNetworkData<B>, ADN>,
    data_provider: impl aleph_bft::DataProvider<AlephData<B>> + Send + 'static,
    ordered_data_interpreter: OrderedDataInterpreter<B, C>,
    backup_saving_path: Option<PathBuf>,
) -> Task {
    let AuthoritySubtaskCommon {
        spawn_handle,
        session_id,
    } = subtask_common;
    let (stop, exit) = oneshot::channel();
    let (saver, loader): (Box<dyn Write + Send>, Box<dyn Read + Send>) =
        if let Some(stash_path) = backup_saving_path.as_deref() {
            let (saver, loader) = rotate_saved_backup_files(stash_path, session_id)
                .expect("Error setting up backup saving");
            (Box::new(saver), Box::new(loader))
        } else {
            (Box::new(io::sink()), Box::new(io::empty()))
        };
    let local_io = LocalIO::new(data_provider, ordered_data_interpreter, saver, loader);
    let task = {
        let spawn_handle = spawn_handle.clone();
        async move {
            debug!(target: "aleph-party", "Running the member task for {:?}", session_id);
            aleph_bft::run_session(config, local_io, network, multikeychain, spawn_handle, exit)
                .await;
            debug!(target: "aleph-party", "Member task stopped for {:?}", session_id);
        }
    };

    let handle = spawn_handle.spawn_essential("aleph/consensus_session_member", task);
    Task::new(handle, stop)
}

fn rotate_saved_backup_files(
    stash_path: &Path,
    session_id: u32,
) -> Result<(File, File), io::Error> {
    let extension = ".abfst";
    let session_path = stash_path.join(format!("{}", session_id));
    fs::create_dir_all(&session_path)?;
    let index = fs::read_dir(&session_path)
        .unwrap()
        .filter_map(|r| r.ok())
        .filter_map(|x| x.file_name().into_string().ok())
        .filter_map(|s| u64::from_str(s.strip_suffix(extension)?).ok())
        .max();
    let load_path = match index {
        Some(index) => session_path.join(format!("{}{}", index, extension)),
        None => "/dev/null".into(),
    };
    let load_file = File::open(load_path)?;
    let save_file =
        File::create(session_path.join(format!("{}{}", index.map_or(0, |i| i + 1), extension)))?;
    Ok((save_file, load_file))
}
