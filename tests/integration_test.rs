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
fn test_nalgebra_sum() {
    let mut jit = JIT::default();
    // Create an array and pass it to sum_array
    // Note: Array literal syntax [1.0, 2.0, ...]
    let code = r#"
    fn test_sum() -> (r: f64) {
        arr = [1.0, 2.0, 3.0, 4.0]
        r = sum_array(arr)
    }
    "#;
    
    let func_ptr = jit.compile(code).unwrap();
    let func: fn() -> f64 = unsafe { std::mem::transmute(func_ptr) };
    
    let result = func();
    assert!((result - 10.0).abs() < 1e-6);
}

#[test]
fn test_nalgebra_print() {
    let mut jit = JIT::default();
    // Just verify it doesn't crash
    let code = r#"
    fn test_print() -> (r: i64) {
        arr = [1.0, 2.0, 3.0, 4.0]
        print_matrix_2x2(arr)
        r = 0
    }
    "#;
    
    let func_ptr = jit.compile(code).unwrap();
    let func: fn() -> i64 = unsafe { std::mem::transmute(func_ptr) };
    
    func();
}

#[test]
fn test_runtime_rand() {
    let mut jit = JIT::default();
    let code = r#"
    fn test_rand() -> (r: i64) {
        r = rand()
    }
    "#;
    
    let func_ptr = jit.compile(code).unwrap();
    let func: fn() -> i64 = unsafe { std::mem::transmute(func_ptr) };
    
    let _val = func();
    // Cannot assert value, but verified it runs
}
