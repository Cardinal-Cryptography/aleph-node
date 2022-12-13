use anyhow::Result;
use relations::{
    note_from_bytes, FrontendAccount, FrontendMerklePathNode, FrontendMerkleRoot, FrontendNote,
};

pub fn parse_frontend_note(frontend_note: &str) -> Result<FrontendNote> {
    Ok(note_from_bytes(frontend_note.as_bytes()))
}

pub fn parse_frontend_merkle_root(frontend_merkle_root: &str) -> Result<FrontendMerkleRoot> {
    Ok(note_from_bytes(frontend_merkle_root.as_bytes()))
}

pub fn parse_frontend_account(frontend_account: &str) -> Result<FrontendAccount> {
    Ok(frontend_account.as_bytes().try_into().unwrap())
}

pub fn parse_frontend_merkle_path_single(
    frontend_merkle_path_single: &str,
) -> Result<FrontendMerklePathNode> {
    Ok(note_from_bytes(frontend_merkle_path_single.as_bytes()))
}
