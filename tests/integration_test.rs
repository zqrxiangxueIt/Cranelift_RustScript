use cranelift_jit_demo::jit::JIT;

#[test]
fn test_math_functions() {
    let mut jit = JIT::default();
    let code = r#"
    fn test_sin(x: f64) -> (r: f64) {
        r = sin(x)
    }
    "#;
    
    let func_ptr = jit.compile(code).unwrap();
    let func: fn(f64) -> f64 = unsafe { std::mem::transmute(func_ptr) };
    
    let result = func(std::f64::consts::PI / 2.0);
    assert!((result - 1.0).abs() < 1e-6);
}

#[test]
fn test_pow() {
    let mut jit = JIT::default();
    let code = r#"
    fn test_pow(b: f64, e: f64) -> (r: f64) {
        r = pow(b, e)
    }
    "#;
    
    let func_ptr = jit.compile(code).unwrap();
    let func: fn(f64, f64) -> f64 = unsafe { std::mem::transmute(func_ptr) };
    
    let result = func(2.0, 3.0);
    assert!((result - 8.0).abs() < 1e-6);
}

#[test]
fn test_mkl_dgemm() {
    let mut jit = JIT::default();
    // Test 2x2 matrix multiplication
    // A = [1, 2; 3, 4], B = [5, 6; 7, 8]
    // C = A * B = [19, 22; 43, 50]
    let code = r#"
    fn test_dgemm(c: [f64; 4]) -> (r: i64) {
        a = [1.0, 2.0, 3.0, 4.0]
        b = [5.0, 6.0, 7.0, 8.0]
        toy_mkl_dgemm(2, 2, 2, 1.0, a, 0.0, b, c)
        r = 0
    }
    "#;
    
    let func_ptr = jit.compile(code).unwrap();
    // The JIT function signature will be: extern "C" fn(*mut f64) -> i64
    // Wait, the JIT function itself ONLY gets the pointer for its own parameters.
    // It doesn't get the length expanded for its OWN parameters.
    let func: fn(*mut f64) -> i64 = unsafe { std::mem::transmute(func_ptr) };
    
    let mut c = [0.0f64; 4];
    func(c.as_mut_ptr());
    
    println!("Resulting matrix C: {:?}", c);
    assert_eq!(c[0], 19.0);
    assert_eq!(c[1], 22.0);
    assert_eq!(c[2], 43.0);
    assert_eq!(c[3], 50.0);
}
