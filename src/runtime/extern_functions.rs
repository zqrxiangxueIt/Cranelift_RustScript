use libc::{c_double};
use rand::Rng;
use std::slice;

// Math functions
#[unsafe(no_mangle)]
pub extern "C" fn toy_sin(x: c_double) -> c_double {
    x.sin()
}

#[unsafe(no_mangle)]
pub extern "C" fn toy_cos(x: c_double) -> c_double {
    x.cos()
}

#[unsafe(no_mangle)]
pub extern "C" fn toy_tan(x: c_double) -> c_double {
    x.tan()
}

#[unsafe(no_mangle)]
pub extern "C" fn toy_sqrt(x: c_double) -> c_double {
    x.sqrt()
}

#[unsafe(no_mangle)]
pub extern "C" fn toy_pow(base: c_double, exp: c_double) -> c_double {
    base.powf(exp)
}

#[unsafe(no_mangle)]
pub extern "C" fn toy_exp(x: c_double) -> c_double {
    x.exp()
}

#[unsafe(no_mangle)]
pub extern "C" fn toy_log(x: c_double) -> c_double {
    x.ln()
}

#[unsafe(no_mangle)]
pub extern "C" fn toy_ceil(x: c_double) -> c_double {
    x.ceil()
}

#[unsafe(no_mangle)]
pub extern "C" fn toy_floor(x: c_double) -> c_double {
    x.floor()
}

// Runtime functions
#[unsafe(no_mangle)]
pub extern "C" fn toy_putchar(c: i64) -> i64 {
    print!("{}", c as u8 as char);
    c
}

#[unsafe(no_mangle)]
pub extern "C" fn toy_rand() -> i64 {
    let mut rng = rand::rng();
    rng.random::<i32>() as i64
}

// Example: Sum elements of a double array
#[unsafe(no_mangle)]
pub unsafe extern "C" fn toy_sum_array(ptr: *const f64, len: usize) -> f64 {
    if ptr.is_null() { return 0.0; }
    let slice = unsafe { slice::from_raw_parts(ptr, len) };
    slice.iter().sum()
}

// Intel MKL Integration

pub type MklInt = i32;

unsafe extern "C" {
    pub fn cblas_dgemm(
        layout: i32,
        trans_a: i32,
        trans_b: i32,
        m: MklInt,
        n: MklInt,
        k: MklInt,
        alpha: f64,
        a: *const f64,
        lda: MklInt,
        b: *const f64,
        ldb: MklInt,
        beta: f64,
        c: *mut f64,
        ldc: MklInt,
    );
}

/// A simplified wrapper for JIT calling
/// layout: 101 (RowMajor), 102 (ColMajor)
/// trans: 111 (NoTrans), 112 (Trans)
#[unsafe(no_mangle)]
pub unsafe extern "C" fn toy_mkl_dgemm(
    m: i64, n: i64, k: i64,
    alpha: f64, a_ptr: *const f64, _a_len: usize,
    beta: f64, b_ptr: *const f64, _b_len: usize,
    c_ptr: *mut f64, _c_len: usize
) {
    unsafe {
        cblas_dgemm(
            101, // CblasRowMajor
            111, // CblasNoTrans
            111, // CblasNoTrans
            m as MklInt,
            n as MklInt,
            k as MklInt,
            alpha,
            a_ptr,
            k as MklInt, // lda (number of columns in A)
            b_ptr,
            n as MklInt, // ldb (number of columns in B)
            beta,
            c_ptr,
            n as MklInt, // ldc (number of columns in C)
        );
    }
}
