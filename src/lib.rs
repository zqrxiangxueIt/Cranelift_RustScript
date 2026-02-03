#[cfg(feature = "mkl")]
extern crate intel_mkl_src;

pub mod frontend;
pub mod jit;
pub mod runtime;
pub mod type_checker;
pub mod cli;
// 导出 raii_demo 供项目内其他模块使用
pub use raii_demo;
