mod validator_node;

pub use validator_node::run_validator_node;

#[cfg(test)]
pub mod testing {
    pub use super::validator_node::new_pen;
}
