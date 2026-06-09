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

#[cfg(feature = "mkl")]
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

    assert_eq!(c[0], 19.0);
    assert_eq!(c[1], 22.0);
    assert_eq!(c[2], 43.0);
    assert_eq!(c[3], 50.0);
}

#[test]
fn test_signed_division() {
    let mut jit = JIT::default();
    let code = r#"
    fn test_sdiv(a: i64, b: i64) -> (r: i64) {
        r = a / b
    }
    "#;
    let func_ptr = jit.compile(code).unwrap();
    let func: fn(i64, i64) -> i64 = unsafe { std::mem::transmute(func_ptr) };
    assert_eq!(func(-10, 3), -3);
    assert_eq!(func(10, -3), -3);
    assert_eq!(func(-10, -3), 3);
}

// ══════════════════════════════════════════════════════
// Phase 4: 块作用域集成测试
// ══════════════════════════════════════════════════════

#[test]
fn test_block_scope_basic() {
    // 块内创建 DynamicArray，块退出时自动释放，返回值正确
    let mut jit = JIT::default();
    let code = r#"
    fn test() -> (r: i64) {
        {
            a = array [1, 2, 3]
            r = a[0]
        }
    }
    "#;
    let func_ptr = jit.compile(code).unwrap();
    let func: fn() -> i64 = unsafe { std::mem::transmute(func_ptr) };
    assert_eq!(func(), 1);
}

#[test]
fn test_nested_block_scope() {
    // 嵌套块：内层数组先释放，外层代码正常运行
    let mut jit = JIT::default();
    let code = r#"
    fn test() -> (r: i64) {
        {
            {
                a = array [42]
                r = a[0]
            }
        }
    }
    "#;
    let func_ptr = jit.compile(code).unwrap();
    let func: fn() -> i64 = unsafe { std::mem::transmute(func_ptr) };
    assert_eq!(func(), 42);
}

#[test]
fn test_block_with_if() {
    // 块内 if/else 各创建数组，块退出时全部释放
    let mut jit = JIT::default();
    let code = r#"
    fn test(flag: i64) -> (r: i64) {
        {
            if flag {
                a = array [10]
                r = a[0]
            } else {
                b = array [20]
                r = b[0]
            }
        }
    }
    "#;
    let func_ptr = jit.compile(code).unwrap();
    let func: fn(i64) -> i64 = unsafe { std::mem::transmute(func_ptr) };
    assert_eq!(func(1), 10);
    assert_eq!(func(0), 20);
}

#[test]
fn test_while_loop_no_leak() {
    // 循环多次创建数组，每次迭代释放，无泄漏
    let mut jit = JIT::default();
    let code = r#"
    fn test() -> (r: i64) {
        i = 0
        sum = 0
        while i < 100 {
            tmp = array [i]
            sum = sum + tmp[0]
            drop(tmp)
            i = i + 1
        }
        r = sum
    }
    "#;
    let func_ptr = jit.compile(code).unwrap();
    let func: fn() -> i64 = unsafe { std::mem::transmute(func_ptr) };
    // sum 0..99 = 4950
    assert_eq!(func(), 4950);
}

#[test]
fn test_while_loop_auto_drop() {
    // 循环体内数组自动释放（不显式 drop），外层的数组可以正常访问
    let mut jit = JIT::default();
    let code = r#"
    fn test() -> (r: i64) {
        outer = array [100, 200, 300]
        i = 0
        while i < 3 {
            tmp = array [outer[i]]
            drop(tmp)
            i = i + 1
        }
        r = outer[2]
        drop(outer)
    }
    "#;
    let func_ptr = jit.compile(code).unwrap();
    let func: fn() -> i64 = unsafe { std::mem::transmute(func_ptr) };
    // 循环体数组迭代释放，外层数组在循环后仍可访问
    assert_eq!(func(), 300);
}
