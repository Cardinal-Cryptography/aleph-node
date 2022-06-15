use aleph_primitives::DEFAULT_UNIT_CREATION_DELAY;
use clap::Parser;
use finality_aleph::UnitCreationDelay;
use std::path::PathBuf;

#[derive(Debug, Parser, Clone)]
pub struct AlephCli {
    #[clap(long)]
    unit_creation_delay: Option<u64>,

    /// The directory to save created backups for crash recovery purposes.
    ///
    /// Backups created by the node are saved under this directory. When restarted after a crash,
    /// previously-created backups are read back from this directory first, helping prevent
    /// auto-forks. The layout of the directory is unspecified.
    #[clap(long, value_name = "PATH")]
    // TOmaybeDO change it to just PathBuf?
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
