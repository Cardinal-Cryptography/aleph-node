use aleph_primitives::DEFAULT_UNIT_CREATION_DELAY;
use finality_aleph::UnitCreationDelay;
use structopt::clap::arg_enum;
use structopt::StructOpt;

arg_enum! {
#[derive(Clone, Debug)]
    enum NodeType {
        NonValidator,
        Validator
    }
}

#[derive(Debug, StructOpt, Clone)]
pub struct AlephCli {
    #[structopt(long)]
    unit_creation_delay: Option<u64>,

    #[structopt(long, possible_values = &NodeType::variants(), case_insensitive = true, default_value = "validator")]
    node_type: NodeType,
}

impl AlephCli {
    pub fn unit_creation_delay(&self) -> UnitCreationDelay {
        UnitCreationDelay(
            self.unit_creation_delay
                .unwrap_or(DEFAULT_UNIT_CREATION_DELAY),
        )
    }

    pub fn node_type(&self) -> finality_aleph::NodeType {
        match self.node_type {
            NodeType::NonValidator => finality_aleph::NodeType::NonValidator,
            NodeType::Validator => finality_aleph::NodeType::Validator,
        }
    }
}
