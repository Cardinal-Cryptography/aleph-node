use clap::Subcommand;
use relations::{
    CircuitField, ConstraintSynthesizer, ConstraintSystemRef, Result as R1CsResult, XorRelation,
};

/// All available relations from `relations` crate.
#[derive(Clone, Eq, PartialEq, Hash, Debug, Subcommand)]
pub enum RelationArgs {
    Xor {
        #[clap(long, short = 'a', default_value = "2")]
        public_xoree: u8,
        #[clap(long, short = 'b', default_value = "3")]
        private_xoree: u8,
        #[clap(long, short = 'c', default_value = "1")]
        result: u8,
    },
    // LinearEquation(LinearEqRelationArgs),
    // MerkleTree(MerkleTreeRelationArgs),
    // Deposit(DepositRelationArgs),
    // Withdraw(WithdrawRelationArgs),
}

impl RelationArgs {
    /// Relation identifier.
    #[allow(dead_code)]
    pub fn id(&self) -> String {
        match &self {
            RelationArgs::Xor { .. } => String::from("xor"),
            // Relation::LinearEquation(_) => String::from("linear_equation"),
            // Relation::MerkleTree(_) => String::from("merkle_tree"),
            // Relation::Deposit(_) => String::from("deposit"),
            // Relation::Withdraw(_) => String::from("withdraw"),
        }
    }
}

impl ConstraintSynthesizer<CircuitField> for RelationArgs {
    fn generate_constraints(self, cs: ConstraintSystemRef<CircuitField>) -> R1CsResult<()> {
        match self {
            RelationArgs::Xor {
                public_xoree,
                private_xoree,
                result,
            } => XorRelation::new(public_xoree, private_xoree, result).generate_constraints(cs),
            // Relation::LinearEquation(relation @ LinearEqRelation { .. }) => {
            //     relation.generate_constraints(cs)
            // }
            // Relation::MerkleTree(args @ MerkleTreeRelationArgs { .. }) => {
            //     <MerkleTreeRelationArgs as Into<MerkleTreeRelation>>::into(args)
            //         .generate_constraints(cs)
            // }
            // Relation::Deposit(args @ DepositRelationArgs { .. }) => {
            //     <DepositRelationArgs as Into<DepositRelation>>::into(args).generate_constraints(cs)
            // }
            // Relation::Withdraw(args @ WithdrawRelationArgs { .. }) => {
            //     <WithdrawRelationArgs as Into<WithdrawRelation>>::into(args)
            //         .generate_constraints(cs)
            // }
        }
    }
}
