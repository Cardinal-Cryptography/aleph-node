use crate::mock::TestExtBuilder;

#[test]
fn empty() {
    let _test = TestExtBuilder::new(vec![0], vec![], 0).build();
}
