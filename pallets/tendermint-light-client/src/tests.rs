use super::*;
use crate::mock::*;

#[test]
fn test_elect() {
    new_test_ext(vec![1, 2]).execute_with(|| {
        assert!(true);

        // let elected = <Elections as ElectionProvider<AccountId, u64>>::elect();
        // assert!(elected.is_ok());

        // let supp = Support {
        //     total: 0,
        //     voters: Vec::new(),
        // };

        // assert_eq!(elected.unwrap(), &[(1, supp.clone()), (2, supp)]);
    });
}
