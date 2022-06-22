use log::{debug, warn};
use std::{
    fmt, fs,
    fs::File,
    io,
    io::{Cursor, Read, Write},
    path::PathBuf,
    str::FromStr,
};

#[derive(Debug)]
pub enum BackupLoadError {
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

pub type Saver = Box<dyn Write + Send>;
pub type Loader = Box<dyn Read + Send>;

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
pub fn rotate(
    backup_path: Option<PathBuf>,
    session_id: u32,
) -> Result<(Saver, Loader), BackupLoadError> {
    const BACKUP_FILE_EXTENSION: &str = ".abfts";

    debug!(target: "aleph-party", "Loading AlephBFT backup for session {:?}", session_id);
    let backup_path = if let Some(path) = backup_path {
        path
    } else {
        debug!(target: "aleph-party", "Passing empty backup for session {:?} as no backup path was provided", session_id);
        return Ok((Box::new(io::sink()), Box::new(io::empty())));
    };
    let session_path = backup_path.join(format!("{}", session_id));
    debug!(target: "aleph-party", "Loading backup for session {:?} at path {:?}", session_id, session_path);

    fs::create_dir_all(&session_path)?;
    let mut session_backups: Vec<_> = fs::read_dir(&session_path)?
        .filter_map(|r| r.ok())
        .filter_map(|x| x.file_name().into_string().ok())
        .filter_map(|s| usize::from_str(s.strip_suffix(BACKUP_FILE_EXTENSION)?).ok())
        .collect();
    session_backups.sort_unstable();
    if !session_backups.iter().cloned().eq(0..session_backups.len()) {
        return Err(BackupLoadError::BackupIncomplete(session_backups));
    }

    let mut buffer = Vec::new();
    for index in session_backups.iter() {
        let load_path = session_path.join(format!("{}{}", index, BACKUP_FILE_EXTENSION));
        File::open(load_path)?.read_to_end(&mut buffer)?;
    }
    let loader = Cursor::new(buffer);

    let session_backup_path = session_path.join(format!(
        "{}{}",
        session_backups.last().map_or(0, |i| i + 1),
        BACKUP_FILE_EXTENSION,
    ));
    debug!(target: "aleph-party", "Loaded backup for session {:?}. Creating new backup file at {:?}", session_id, session_backup_path);
    let saver = File::create(session_backup_path)?;

    debug!(target: "aleph-party", "Backup rotation done for session {:?}", session_id);
    Ok((Box::new(saver), Box::new(loader)))
}

/// Removes the backup directory for a session.
///
/// `backup_path` is the path to the backup directory (i.e. the argument to `--backup-saving-path`).
/// If it is `None`, nothing is done.
///
/// Any filesystem errors are logged and dropped.
///
/// This should be done after the end of the session.
pub fn remove(path: Option<PathBuf>, session_id: u32) {
    let path = match path {
        Some(path) => path.join(session_id.to_string()),
        None => return,
    };
    match fs::remove_dir_all(&path) {
        Ok(()) => {
            debug!(target: "aleph-party", "Removed backup for session {}", session_id);
        }
        Err(error) => {
            warn!(target: "aleph-party", "Error cleaning up backup for session {}: {}", session_id, error);
        }
    }
}
