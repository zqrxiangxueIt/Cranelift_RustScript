#[cfg(feature = "mkl")]
extern crate intel_mkl_src;

pub mod cli;
pub mod frontend;
pub mod jit;
pub mod optimizer;
pub mod ownership;
pub mod runtime;
pub mod type_checker;
