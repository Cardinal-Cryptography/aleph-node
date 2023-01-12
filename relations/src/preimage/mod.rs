mod relation;
// #[cfg(test)]
mod tests;

pub use relation::PreimageRelation;
// #[cfg(any(test, bench))]
pub use tests::preimage_proving_and_verifying;
