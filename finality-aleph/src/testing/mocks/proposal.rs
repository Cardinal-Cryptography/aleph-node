use crate::{
    block::{Block, UnverifiedHeader},
    data_io::{AlephData, UnvalidatedAlephProposal},
};

pub fn unvalidated_proposal_from_headers<U: UnverifiedHeader>(
    mut headers: Vec<U>,
) -> UnvalidatedAlephProposal<U> {
    let head = headers.pop().unwrap();
    let tail = headers
        .into_iter()
        .map(|header| header.id().hash())
        .collect();
    UnvalidatedAlephProposal::new(head, tail)
}

pub fn aleph_data_from_blocks<B: Block>(blocks: Vec<B>) -> AlephData<B::UnverifiedHeader> {
    let headers = blocks.into_iter().map(|b| b.header().clone()).collect();
    aleph_data_from_headers(headers)
}

pub fn aleph_data_from_headers<U: UnverifiedHeader>(headers: Vec<U>) -> AlephData<U> {
    AlephData {
        head_proposal: unvalidated_proposal_from_headers(headers),
    }
}
