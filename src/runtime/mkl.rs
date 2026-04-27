#[cfg(feature = "mkl")]
pub type MklInt = i32;

#[cfg(feature = "mkl")]
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

#[cfg(feature = "mkl")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn toy_mkl_dgemm(
    m: i64,
    n: i64,
    k: i64,
    alpha: f64,
    a_ptr: *const f64,
    a_len: usize,
    beta: f64,
    b_ptr: *const f64,
    b_len: usize,
    c_ptr: *mut f64,
    c_len: usize,
) -> i64 {
    // Validate array sizes: A is m x k, B is k x n, C is m x n
    let required_a = (m as usize) * (k as usize);
    let required_b = (k as usize) * (n as usize);
    let required_c = (m as usize) * (n as usize);

    if a_len < required_a {
        return -1; // Error: a_len insufficient (a_len < m * k)
    }
    if b_len < required_b {
        return -2; // Error: b_len insufficient (b_len < k * n)
    }
    if c_len < required_c {
        return -3; // Error: c_len insufficient (c_len < m * n)
    }

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
            k as MklInt, // lda
            b_ptr,
            n as MklInt, // ldb
            beta,
            c_ptr,
            n as MklInt, // ldc
        );
    }
    0 // Success
}
