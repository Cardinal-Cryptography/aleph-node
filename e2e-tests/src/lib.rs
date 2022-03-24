use substrate_api_client::{AccountId, UncheckedExtrinsicV4};
// TODO remove `pub`
pub mod accounts;
pub mod config;
mod fee;
pub mod staking;
pub mod test;
pub mod transfer;
mod waiting;

#[macro_export]
macro_rules! send_extrinsic_no_wait {
	($connection: expr,
	$module: expr,
	$call: expr
	$(, $args: expr) *) => {
		{
            use substrate_api_client::{compose_extrinsic, UncheckedExtrinsicV4, XtStatus};

            let tx: UncheckedExtrinsicV4<_> = compose_extrinsic!(
                $connection,
                $module,
                $call
                $(, ($args)) *
            );

            let _ = $connection
                .send_extrinsic(tx.hex_encode(), XtStatus::InBlock)
                .unwrap();
		}
    };
}
