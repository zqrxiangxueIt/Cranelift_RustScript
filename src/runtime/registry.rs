use cranelift_jit::JITBuilder;
use crate::runtime::{io, math, string};

#[cfg(feature = "mkl")]
use crate::runtime::mkl;

/// Macro to define a runtime function with C ABI and no mangling.
#[macro_export]
macro_rules! runtime_fn {
    (fn $name:ident($($arg:ident: $arg_ty:ty),*) -> $ret_ty:ty $body:block) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $name($($arg: $arg_ty),*) -> $ret_ty $body
    };
}

/// Registers all built-in functions to the JITBuilder.
pub fn register_builtins(builder: &mut JITBuilder) {
    // Register basic IO and runtime functions
    builder.symbol("printf", string::printf as *const u8);
    builder.symbol("puts", string::puts as *const u8);
    builder.symbol("putchar", io::toy_putchar as *const u8);
    builder.symbol("rand", io::toy_rand as *const u8);
    builder.symbol("toy_sum_array", io::toy_sum_array as *const u8);

    // Register math functions
    builder.symbol("sin", math::toy_sin as *const u8);
    builder.symbol("cos", math::toy_cos as *const u8);
    builder.symbol("tan", math::toy_tan as *const u8);
    builder.symbol("sqrt", math::toy_sqrt as *const u8);
    builder.symbol("pow", math::toy_pow as *const u8);
    builder.symbol("exp", math::toy_exp as *const u8);
    builder.symbol("log", math::toy_log as *const u8);
    builder.symbol("ceil", math::toy_ceil as *const u8);
    builder.symbol("floor", math::toy_floor as *const u8);

    // Feature-gated registration packages
    #[cfg(feature = "mkl")]
    register_mkl(builder);

    #[cfg(feature = "gpu")]
    register_gpu(builder);
}

/// Pre-set package for MKL functions.
#[cfg(feature = "mkl")]
pub fn register_mkl(builder: &mut JITBuilder) {
    unsafe {
        builder.symbol("cblas_dgemm", mkl::cblas_dgemm as *const u8);
        builder.symbol("toy_mkl_dgemm", mkl::toy_mkl_dgemm as *const u8);
    }
}

/// Pre-set package for GPU functions.
#[cfg(feature = "gpu")]
pub fn register_gpu(_builder: &mut JITBuilder) {
    // Placeholder for CUDA/GPU interface registration
}

/// Minimum set for basic arithmetic and memory (default).
pub fn register_minimal(builder: &mut JITBuilder) {
    builder.symbol("putchar", io::toy_putchar as *const u8);
    builder.symbol("rand", io::toy_rand as *const u8);
}
