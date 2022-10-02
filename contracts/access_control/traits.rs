use ink_env::{
    call::{build_call, Call, ExecutionInput, Selector},
    AccountId, DefaultEnvironment, Error as InkEnvError,
};

use crate::{access_control::HAS_ROLE_SELECTOR, roles::Role};

pub trait AccessControlled {
    type ContractError;

    fn check_role<ContractError>(
        access_control: AccountId,
        account: AccountId,
        role: Role,
        contract_call_error_handler: fn(why: InkEnvError) -> ContractError,
        access_control_error_handler: fn(role: Role) -> ContractError,
    ) -> Result<(), ContractError> {
        match build_call::<DefaultEnvironment>()
            .call_type(Call::new().callee(access_control))
            .exec_input(
                ExecutionInput::new(Selector::new(HAS_ROLE_SELECTOR))
                    .push_arg(account)
                    .push_arg(role),
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
