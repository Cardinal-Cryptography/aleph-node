use frame_benchmarking::Vec;

pub(super) struct Artifacts {
    pub key: Vec<u8>,
    pub proof: Vec<u8>,
    pub input: Vec<u8>,
}

#[macro_export]
macro_rules! get_artifacts {
    ($(ProvingSystem::)?$system:tt, $(Relation::)?$relation:tt $(,)?) => {{
        let key = $crate::get_artifact!($system, $relation, VerifyingKey);
        let proof = $crate::get_artifact!($system, $relation, Proof);
        let input = $crate::get_artifact!($system, $relation, PublicInput);

        $crate::benchmarking::import::Artifacts { key, proof, input }
    }};
}

#[macro_export]
macro_rules! get_artifact {
    ($(ProvingSystem::)?$system:tt, $(Relation::)?$relation:tt, $(Artifact::)?$artifact:tt $(,)?) => {
        include_bytes!(concat!(
            "resources/",
            $crate::system!($system),
            "/",
            $crate::relation!($relation),
            ".",
            $crate::artifact!($artifact),
            ".bytes"
        ))
        .to_vec()
    };
}

#[macro_export]
macro_rules! system {
    (Groth16) => {
        "groth16"
    };
    (Gm17) => {
        "gm17"
    };
}

#[macro_export]
macro_rules! relation {
    (Xor) => {
        "xor"
    };
    (LinearEquation) => {
        "linear_equation"
    };
    (MerkleTree) => {
        "merkle_tree"
    };
}

#[macro_export]
macro_rules! artifact {
    (VerifyingKey) => {
        "vk"
    };
    (Proof) => {
        "proof"
    };
    (PublicInput) => {
        "public_input"
    };
}
