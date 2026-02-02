use libc::{c_double};
use rand::Rng;
use nalgebra::{SMatrix};
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

// Nalgebra wrapper examples

// Example: Sum elements of a double array
#[unsafe(no_mangle)]
pub unsafe extern "C" fn toy_sum_array(ptr: *const f64, len: usize) -> f64 {
    if ptr.is_null() { return 0.0; }
    let slice = unsafe { slice::from_raw_parts(ptr, len) };
    slice.iter().sum()
}

// Example: Print a matrix (assuming column-major if using nalgebra)
// For this demo, let's assume we pass a 2x2 matrix as an array of 4 doubles.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn toy_print_matrix_2x2(ptr: *const f64, len: usize) {
    if len != 4 {
        println!("Error: Expected 4 elements for 2x2 matrix, got {}", len);
        return;
    }
    let slice = unsafe { slice::from_raw_parts(ptr, len) };
    // Construct SMatrix from slice (column-major by default in nalgebra)
    let mat = SMatrix::<f64, 2, 2>::from_column_slice(slice);
    println!("Matrix 2x2:\n{}", mat);
}
