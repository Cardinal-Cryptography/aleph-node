mod import;
mod suite;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
enum Relation {
    Xor,
    LinearEquation,
    MerkleTree,
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
enum Artifact {
    VerifyingKey,
    Proof,
    PublicInput,
}
