use crate::types::{
    LightBlockStorage, TendermintPeerId, TimestampStorage, ValidatorInfoStorage,
    ValidatorSetStorage,
};
use codec::alloc::string::ToString;
use scale_info::prelude::string::String;
use sp_std::vec::Vec;
use tendermint_testgen as testgen;

pub fn generate_consecutive_blocks(
    n: usize,
    chain_id: String,
    validators_count: u32,
    from_height: u64,
    from_timestamp: TimestampStorage,
) -> Vec<LightBlockStorage> {
    let validators = (0..validators_count)
        .map(|id| testgen::Validator::new(&id.to_string()).voting_power(50))
        .collect::<Vec<testgen::Validator>>();

    let header = testgen::Header::new(&validators)
        .height(from_height)
        .chain_id(&chain_id)
        .next_validators(&validators)
        .time(from_timestamp.try_into().unwrap());

    let commit = testgen::Commit::new(header.clone(), 1);

    let validators = testgen::validator::generate_validators(&validators)
        .unwrap()
        .into_iter()
        .map(|v| v.try_into().unwrap())
        .collect::<Vec<ValidatorInfoStorage>>();

    let validators_set = ValidatorSetStorage::new(validators, None, 50 * validators_count as u64);

    let signed_header = testgen::light_block::generate_signed_header(&header, &commit).unwrap();

    let default_provider = TendermintPeerId::from_slice(&[
        186, 223, 173, 173, 11, 239, 238, 220, 12, 10, 222, 173, 190, 239, 192, 255, 238, 250, 202,
        222,
    ]);

    let mut block = testgen::LightBlock::new(header, commit);
    let mut blocks = Vec::with_capacity(n);

    let block_storage = LightBlockStorage::new(
        signed_header.clone().try_into().unwrap(),
        validators_set.clone(),
        validators_set.clone(),
        default_provider,
    );

    blocks.push(block_storage);

    for _index in 1..n {
        block = block.next();

        let testgen::LightBlock { header, commit, .. } = block.clone();
        let signed_header = testgen::light_block::generate_signed_header(
            &header.clone().unwrap(),
            &commit.unwrap(),
        )
        .unwrap();

        let bs = LightBlockStorage::new(
            signed_header.try_into().unwrap(),
            validators_set.clone(),
            validators_set.clone(),
            default_provider,
        );

        blocks.push(bs);
    }

    blocks.reverse();

    // TODO
    // blocks.iter().for_each(|b| {
    //     println!(
    //         "block {:?} \ntimestamp {:?} \nprevious block: {:?}",
    //         b.signed_header.commit.block_id.hash,
    //         b.signed_header.header.timestamp,
    //         b.signed_header.header.last_block_id,
    //     );
    //     println!();
    // });

    blocks
}
