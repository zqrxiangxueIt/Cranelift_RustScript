use core::mem;
use cranelift_jit_demo::jit;

fn main() -> Result<(), String> {
    // Program main function, implementing toy language execution
    let mut jit = jit::JIT::default();
    
    println!("foo(1, 0) = {}", run_foo(&mut jit)?);
    
    println!(
        "recursive_fib(10) = {}",
        run_recursive_fib(&mut jit, 10)?
    );
    
    println!(
        "iterative_fib(10) = {}",
        run_iterative_fib(&mut jit, 10)?
    );
    
    println!(
        "float_add(1.5, 2.5) = {}", 
        run_float_add(&mut jit, 1.5, 2.5)?
    );
    
    println!(
        "mixed_add(10, 2.5) = {}", 
        run_mixed_add(&mut jit, 10, 2.5, 2.5)?
    );
    
    run_hello(&mut jit)?;


    println!(
        "mul_div(10.0, 5.0) = {}",
        run_mul_div(&mut jit, 10.0, 5.0)?
    );

    run_custom_string(&mut jit, "Customize String Test Success!")?;
    
    // --- New Tests ---
    
    println!("--- Running String Literal Test ---");
    run_string_test(&mut jit)?;

    println!("--- Running I128 Test ---");
    run_i128_test(&mut jit)?;

    println!("--- Running Complex Test ---");
    run_complex_test(&mut jit)?;
    
    println!("--- Running Array Test ---");
    run_array_test(&mut jit)?;

    Ok(())
}

fn run_i128_test(jit: &mut jit::JIT) -> Result<i64, String> {
    unsafe {
        let code_ptr = jit.compile(I128_TEST_CODE)?;
        let code_fn = mem::transmute::<_, extern "C" fn() -> i64>(code_ptr);
        Ok(code_fn())
    }
}

const I128_TEST_CODE: &str = r#"
    fn i128_test() -> (r: i64) {
        x = 100 as i128
        r = x as i64
    }
"#;

fn run_foo(jit: &mut jit::JIT) -> Result<i64, String> {
    unsafe {
        let code_ptr = jit.compile(FOO_CODE)?;
        let code_fn = mem::transmute::<_, extern "C" fn(i64, i64) -> i64>(code_ptr);
        Ok(code_fn(1, 0))  
    }
}

fn run_recursive_fib(jit: &mut jit::JIT, input: i64) -> Result<i64, String> {
    unsafe {
        let code_ptr = jit.compile(RECURSIVE_FIB_CODE)?;
        let code_fn = mem::transmute::<_, extern "C" fn(i64) -> i64>(code_ptr);
        Ok(code_fn(input))
    }
}
 
fn run_iterative_fib(jit: &mut jit::JIT, input: i64) -> Result<i64, String> {
    unsafe {
        let code_ptr = jit.compile(ITERATIVE_FIB_CODE)?;
        let code_fn = mem::transmute::<_, extern "C" fn(i64) -> i64>(code_ptr);
        Ok(code_fn(input))
    }
}

fn run_float_add(jit: &mut jit::JIT, a: f64, b: f64) -> Result<f64, String> {
    unsafe {
        let code_ptr = jit.compile(FLOAT_ADD_CODE)?;
        let code_fn = mem::transmute::<_, extern "C" fn(f64, f64) -> f64>(code_ptr);
        Ok(code_fn(a, b))
    }
}

fn run_mixed_add(jit: &mut jit::JIT, a: i32, b: f64, c: f64) -> Result<f64, String> {
    unsafe {
        let code_ptr = jit.compile(MIXED_ADD_CODE)?;
        let code_fn = mem::transmute::<_, extern "C" fn(i32, f64, f64) -> f64>(code_ptr);
        Ok(code_fn(a, b, c))
    }
}

fn run_hello(jit: &mut jit::JIT) -> Result<i64, String> {
    jit.create_data("hello_string", "hello world!\0".as_bytes().to_vec())?;
    unsafe {
        let code_ptr = jit.compile(HELLO_CODE)?;
        let code_fn = mem::transmute::<_, extern "C" fn() -> i64>(code_ptr);
        Ok(code_fn())
    }
}

fn run_mul_div(jit: &mut jit::JIT, a: f64, b: f64) -> Result<f64, String> {
    unsafe {
        let code_ptr = jit.compile(MUL_DIV_CODE)?;
        let code_fn = mem::transmute::<_, extern "C" fn(f64, f64) -> f64>(code_ptr);
        Ok(code_fn(a, b))
    }
}

fn run_custom_string(jit: &mut jit::JIT, msg: &str) -> Result<i64, String> {
    let mut msg_bytes = msg.as_bytes().to_vec();
    msg_bytes.push(0); // Null terminator
    jit.create_data("custom_msg", msg_bytes)?;
    unsafe {
        let code_ptr = jit.compile(CUSTOM_STRING_CODE)?;
        let code_fn = mem::transmute::<_, extern "C" fn() -> i64>(code_ptr);
        Ok(code_fn())
    }
}

fn run_string_test(jit: &mut jit::JIT) -> Result<i64, String> {
    unsafe {
        let code_ptr = jit.compile(STRING_TEST_CODE)?;
        let code_fn = mem::transmute::<_, extern "C" fn() -> i64>(code_ptr);
        Ok(code_fn())
    }
}

fn run_complex_test(jit: &mut jit::JIT) -> Result<i64, String> {
    unsafe {
        let code_ptr = jit.compile(COMPLEX_TEST_CODE)?;
        // Returns status (i64)
        let code_fn = mem::transmute::<_, extern "C" fn() -> i64>(code_ptr);
        let result = code_fn();
        
        println!("Complex Test Status: {}", result);
        
        if result == 1 {
             Ok(result)
        } else {
             Err(format!("Complex test failed"))
        }
    }
}

fn run_array_test(jit: &mut jit::JIT) -> Result<i64, String> {
     unsafe {
        let code_ptr = jit.compile(ARRAY_TEST_CODE)?;
        let code_fn = mem::transmute::<_, extern "C" fn() -> i64>(code_ptr);
        let result = code_fn();
        println!("Array test result: {}", result);
        if result == 30 {
            Ok(result)
        } else {
            Err(format!("Array test failed: expected 30, got {}", result))
        }
    }
}

const FOO_CODE: &str = r#"
    fn foo(a: i64, b: i64) -> (c: i64) {
        c = if a {
            if b {
                30
            } else {
                40
            }
        } else {
            50
        }
        c = c + 2
    }
"#;

const RECURSIVE_FIB_CODE: &str = r#"
    fn recursive_fib(n: i64) -> (r: i64) {
        r = if n == 0 {
                    0
            } else {
                if n == 1 {
                    1
                } else {
                    recursive_fib(n - 1) + recursive_fib(n - 2)
                }
            }
    }
"#;

const ITERATIVE_FIB_CODE: &str = r#"
    fn iterative_fib(n: i64) -> (r: i64) {
        if n == 0 {
            r = 0
        } else {
            n = n - 1
            a = 0
            r = 1
            while n != 0 {
                t = r
                r = r + a
                a = t
                n = n - 1
            }
        }
    }
"#;

const FLOAT_ADD_CODE: &str = r#"
    fn float_add(a: f64, b: f64) -> (c: f64) {
        c = a + b
    }
"#;

const MIXED_ADD_CODE: &str = r#"
    fn mixed_add(a: i32, b: f64, c: f64) -> (r: f64) {
        r = (a as f64) + b + c
    }
"#;

const HELLO_CODE: &str = r#"
fn hello() -> (r: i64) {
    puts(&hello_string)
}
"#;

const MUL_DIV_CODE: &str = r#"
fn mul_div(a: f64, b: f64) -> (c: f64) {
    c = a * b / 2.0
}
"#;

const CUSTOM_STRING_CODE: &str = r#"
fn custom_string_func() -> (r: i64) {
    puts(&custom_msg)
}
"#;

const STRING_TEST_CODE: &str = r#"
fn string_test() -> (r: i64) {
    s = "Hello from JIT String Literal!\nWith Newline\tAnd Tab"
    puts(s)
    fmt = "Printf Test: %s %d\n"
    world = "World"
    num = 123
    printf(fmt, world, num)
    r = 0
}
"#;

const COMPLEX_TEST_CODE: &str = r#"
    fn complex_test() -> (status: i64) {
        c1 = 1.5 + 2.5i
        c2 = 0.5 + 0.5i
        c = c1 + c2
        status = 1
    }
"#;

const ARRAY_TEST_CODE: &str = r#"
fn array_test() -> (r: i64) {
    arr = [10, 20, 30] 
    x = arr[2]
    r = x
}
"#;
