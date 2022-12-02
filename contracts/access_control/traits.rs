use ink::env::{
    call::{build_call, Call, ExecutionInput, Selector},
    Environment, Error as InkEnvError,
};

use crate::{access_control::HAS_ROLE_SELECTOR, roles::Role};

/// Convenience trait for contracts that have methods that need to be under access control
///
/// Such contracts should implement this trait and pass their error enum as associated type:
/// impl AccessControlled for MyContract {
///     type ContractError = MyContractError;
/// }
pub trait AccessControlled<E: Environment> {
    type ContractError;

    fn check_role<ContractError>(
        access_control: E::AccountId,
        account: E::AccountId,
        role: Role<E>,
        contract_call_error_handler: fn(why: InkEnvError) -> ContractError,
        access_control_error_handler: fn(role: Role<E>) -> ContractError,
    ) -> Result<(), ContractError> {
        match build_call::<E>()
            .call_type(Call::new().callee(access_control))
            .exec_input(
                ExecutionInput::new(Selector::new(HAS_ROLE_SELECTOR))
                    .push_arg(account)
                    .push_arg(&role),
            )
            .returns::<bool>()
            .fire()
        {
            Ok(has_role) => match has_role {
                true => Ok(()),
                false => Err(access_control_error_handler(role)),
            },
            Err(why) => Err(contract_call_error_handler(why)),
        }
    }
}
