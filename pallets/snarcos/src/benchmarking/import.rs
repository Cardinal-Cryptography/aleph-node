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
