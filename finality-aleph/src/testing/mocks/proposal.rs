use crate::{
    block::mock::{MockBlock, MockHeader},
    data_io::{AlephData, UnvalidatedAlephProposal},
};

pub fn unvalidated_proposal_from_headers(
    mut headers: Vec<MockHeader>,
) -> UnvalidatedAlephProposal<MockHeader> {
    let head = headers.pop().unwrap();
    let tail = headers.into_iter().map(|header| header.hash()).collect();
    UnvalidatedAlephProposal::new(head, tail)
}

pub fn aleph_data_from_blocks(blocks: Vec<MockBlock>) -> AlephData<MockHeader> {
    let headers = blocks.into_iter().map(|b| b.header().clone()).collect();
    aleph_data_from_headers(headers)
}

pub fn aleph_data_from_headers(headers: Vec<MockHeader>) -> AlephData<MockHeader> {
    AlephData {
        head_proposal: unvalidated_proposal_from_headers(headers),
    }
}
