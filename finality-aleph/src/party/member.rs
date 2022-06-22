use crate::{
    crypto::KeyBox,
    data_io::{AlephData, OrderedDataInterpreter},
    network::{AlephNetworkData, DataNetwork, NetworkWrapper},
    party::{AuthoritySubtaskCommon, Task},
};
use aleph_bft::{Config, LocalIO, SpawnHandle};
use futures::channel::oneshot;
use log::{debug, error};
use sc_client_api::HeaderBackend;
use sp_runtime::traits::Block;
use std::{
    fmt, fs,
    fs::File,
    io,
    io::{Cursor, Read, Write},
    path::PathBuf,
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
    let task = {
        let spawn_handle = spawn_handle.clone();
        async move {
            let (saver, loader) = match rotate_saved_backup_files(backup_saving_path, session_id) {
                Ok((saver, loader)) => (saver, loader),
                Err(err) => {
                    error!(
                        target: "AlephBFT-member",
                        "Error setting up backup saving for session {}: {}",
                        session_id, err
                    );
                    return;
                }
            };
            let local_io = LocalIO::new(data_provider, ordered_data_interpreter, saver, loader);
            debug!(target: "aleph-party", "Running the member task for {:?}", session_id);
            aleph_bft::run_session(config, local_io, network, multikeychain, spawn_handle, exit)
                .await;
            debug!(target: "aleph-party", "Member task stopped for {:?}", session_id);
        }
    };

    let handle = spawn_handle.spawn_essential("aleph/consensus_session_member", task);
    Task::new(handle, stop)
}

#[derive(Debug)]
enum BackupLoadError {
    BackupIncomplete(Vec<usize>),
    IOError(io::Error),
}

impl fmt::Display for BackupLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BackupLoadError::BackupIncomplete(backups) => {
                write!(
                    f,
                    "Backup is not complete. Got backup for runs numbered: {:?}",
                    backups
                )
            }
            BackupLoadError::IOError(err) => {
                write!(f, "Backup could not be loaded because of IO error: {}", err)
            }
        }
    }
}

impl From<io::Error> for BackupLoadError {
    fn from(err: io::Error) -> Self {
        Self::IOError(err)
    }
}

impl std::error::Error for BackupLoadError {}

/// Loads the existing backups, and opens a new backup file to write to.
///
/// `backup_path` is the path to the backup directory (i.e. the argument to `--backup-saving-path`).
///
/// Returns the newly-created file (opened for writing), and the concatenation of the contents of
/// all existing files.
///
/// Current directory structure (this is an implementation detail, not part of the public API):
///   backup-stash/      - the main directory, backup_path/--backup-saving-path
///   `-- 18723/         - subdirectory for the current session
///       |-- 0.abfts    - files containing data
///       |-- 1.abfts    - each restart after a crash will cause another one to be created
///       |-- 2.abfts    - these numbers count up sequentially
///       `-- 3.abfts
fn rotate_saved_backup_files(
    backup_path: Option<PathBuf>,
    session_id: u32,
) -> Result<(Box<dyn Write + Send>, Box<dyn Read + Send>), BackupLoadError> {
    debug!(target: "aleph-party", "Loading AlephBFT backup for session {:?}", session_id);
    let backup_path = if let Some(path) = backup_path {
        path
    } else {
        debug!(target: "aleph-party", "Passing empty backup for session {:?} as no backup path was provided", session_id);
        return Ok((Box::new(io::sink()), Box::new(io::empty())));
    };
    let extension = ".abfts";
    let session_path = backup_path.join(format!("{}", session_id));
    debug!(target: "aleph-party", "Loading backup for session {:?} at path {:?}", session_id, session_path);
    fs::create_dir_all(&session_path)?;
    let mut session_backups: Vec<_> = fs::read_dir(&session_path)?
        .filter_map(|r| r.ok())
        .filter_map(|x| x.file_name().into_string().ok())
        .filter_map(|s| usize::from_str(s.strip_suffix(extension)?).ok())
        .collect();
    session_backups.sort_unstable();
    if !session_backups.iter().cloned().eq(0..session_backups.len()) {
        return Err(BackupLoadError::BackupIncomplete(session_backups));
    }
    let mut buffer = Vec::new();
    for index in session_backups.iter() {
        let load_path = session_path.join(format!("{}{}", index, extension));
        let _ = File::open(load_path)?.read_to_end(&mut buffer)?;
    }
    let loader = Cursor::new(buffer);
    let session_backup_path = session_path.join(format!(
        "{}{}",
        session_backups.last().map_or(0, |i| i + 1),
        extension
    ));
    debug!(target: "aleph-party", "Loaded backup for session {:?}. Creating new backup file at {:?}", session_id, session_backup_path);
    let saver = File::create(session_backup_path)?;
    debug!(target: "aleph-party", "Backup rotation done for session {:?}", session_id);
    Ok((Box::new(saver), Box::new(loader)))
}
