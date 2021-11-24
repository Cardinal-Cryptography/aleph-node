use crate::crypto::{Signature, SignatureV1};
use crate::justification::{
    backwards_compatible_decode, AlephJustification, AlephJustificationV1, JustificationDecoding,
};
use aleph_bft::{PartialMultisignature, SignatureSet};
use aleph_primitives::AuthoritySignature;
use codec::Encode;

#[test]
fn correctly_decodes_v1() {
    let mut signature_set: SignatureSet<SignatureV1> = SignatureSet::with_size(7.into());
    for i in 0..7 {
        let id = i.into();
        let signature_v1 = SignatureV1 {
            _id: id,
            sgn: Default::default(),
        };
        signature_set = signature_set.add_signature(&signature_v1, id);
    }

    let just_v1 = AlephJustificationV1 {
        signature: signature_set,
    };
    let encoded_just: Vec<u8> = just_v1.encode();
    let decoded = backwards_compatible_decode(encoded_just);
    assert_eq!(decoded, JustificationDecoding::V1(just_v1));
}

#[test]
fn correctly_decodes_v2() {
    let mut signature_set: SignatureSet<Signature> = SignatureSet::with_size(7.into());
    for i in 0..7 {
        let authority_signature: AuthoritySignature = Default::default();
        signature_set = signature_set.add_signature(&authority_signature.into(), i.into());
    }

    let just_v2 = AlephJustification {
        signature: signature_set,
    };
    let encoded_just: Vec<u8> = just_v2.encode();
    let decoded = backwards_compatible_decode(encoded_just);
    assert_eq!(decoded, JustificationDecoding::V2(just_v2),);
}
