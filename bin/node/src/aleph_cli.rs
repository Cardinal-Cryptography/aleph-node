use aleph_primitives::DEFAULT_UNIT_CREATION_DELAY;
use clap::Parser;
use finality_aleph::UnitCreationDelay;
use std::path::PathBuf;

#[derive(Debug, Parser, Clone)]
pub struct AlephCli {
    #[clap(long)]
    unit_creation_delay: Option<u64>,

    /// The directory to save created units to for crash recovery purposes.
    ///
    /// Units created by the node are saved under this directory. When restarted after a crash,
    /// previously-created units are read back from this directory first, helping prevent
    /// auto-forks. The layout of the directory is unspecified.
    #[clap(
        long,
        value_name = "PATH",
        required_unless_present("unit-saving"),
        conflicts_with("unit-saving")
    )]
    unit_saving_path: Option<PathBuf>,
    #[clap(long = "no-unit-saving", parse(from_flag = ignore))]
    _unit_saving: (),
}

impl AlephCli {
    pub fn unit_creation_delay(&self) -> UnitCreationDelay {
        UnitCreationDelay(
            self.unit_creation_delay
                .unwrap_or(DEFAULT_UNIT_CREATION_DELAY),
        )
    }

    pub fn unit_saving_path(&self) -> Option<PathBuf> {
        self.unit_saving_path.clone()
    }
}

fn ignore<T>(_: T) {}
