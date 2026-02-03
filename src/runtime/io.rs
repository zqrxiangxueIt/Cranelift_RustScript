use rand::Rng;
use std::slice;

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

#[unsafe(no_mangle)]
pub unsafe extern "C" fn toy_sum_array(ptr: *const f64, len: usize) -> f64 {
    if ptr.is_null() { return 0.0; }
    let slice = unsafe { slice::from_raw_parts(ptr, len) };
    slice.iter().sum()
}
