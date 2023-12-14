use frame_benchmarking::v2::*;
use sp_std::vec;

trait Config {}
impl<T> Config for T {}

/// todo
pub struct Pallet<T> {
    _phantom: sp_std::marker::PhantomData<T>,
}
/// todo
pub type ChainExtensionBenchmarking<T> = Pallet<T>;

#[benchmarks]
mod benchmarks {
    use super::*;

    #[benchmark]
    fn reading_arguments() {
        #[block]
        {
            todo!()
        }
    }
}
