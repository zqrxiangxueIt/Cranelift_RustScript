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
            k as MklInt, // lda
            b_ptr,
            n as MklInt, // ldb
            beta,
            c_ptr,
            n as MklInt, // ldc
        );
    }
}
