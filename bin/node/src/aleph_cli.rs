use aleph_primitives::DEFAULT_UNIT_CREATION_DELAY;
use clap::{ArgGroup, Parser};
use finality_aleph::UnitCreationDelay;
use std::path::PathBuf;

#[derive(Debug, Parser, Clone)]
#[clap(group(ArgGroup::new("backup").required(true)))]
pub struct AlephCli {
    #[clap(long)]
    unit_creation_delay: Option<u64>,

    #[clap(long, value_name = "PRUNING_MODE", required = true)]
    pub pruning: Option<String>,

    /// This flags needs to be provided in case used does not want to create backups.
    /// In case `--no-backup`, node most likely will not be available to continue with the
    /// session during which it crashed. It will join AlephBFT consensus in the next session.
    #[clap(long, conflicts_with = "backup-path", group = "backup")]
    no_backup: bool,
    /// The path to save created backups for crash recovery purposes.
    ///
    /// Backups created by the node are saved under this  path in a directory. When restarted after a crash,
    /// previously-created backups are read back from this directory first, helping prevent
    /// auto-forks. The layout of the directory is unspecified. User is required to provide this path,
    /// or explicitly say that no backups should be done by providing `--no-backup` flag.
    /// In case no backups are c, node most likely will not be available to continue with the
    #[clap(
        long,
        value_name = "PATH",
        conflicts_with = "no-backup",
        group = "backup"
    )]
    backup_path: Option<PathBuf>,
}

impl AlephCli {
    pub fn unit_creation_delay(&self) -> UnitCreationDelay {
        UnitCreationDelay(
            self.unit_creation_delay
                .unwrap_or(DEFAULT_UNIT_CREATION_DELAY),
        )
    }

    pub fn backup_path(&self) -> Option<PathBuf> {
        self.backup_path.clone()
    }
}
