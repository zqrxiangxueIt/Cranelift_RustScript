use anyhow::{Context, Result, anyhow};
use cranelift_jit_demo::jit;
use cranelift_jit_demo::cli::Cli;
use std::fs;
use std::mem;
use std::path::Path;

fn main() -> Result<()> {
    let cli = Cli::parse_args();

    if cli.test {
        println!("Running integration tests...");
        run_all_tests().context("Integration tests failed")?;
        println!("All tests passed!");
    } else if let Some(file_path) = cli.file {
        run_script(&file_path).with_context(|| format!("Failed to run script: {:?}", file_path))?;
    } else {
        use clap::CommandFactory;
        Cli::command().print_help()?;
        println!();
    }

    Ok(())
}

fn run_script(path: &Path) -> Result<()> {
    // 1. Verify file existence and extension
    if !path.exists() {
        return Err(anyhow!("File not found: {:?}", path));
    }
    if path.extension().and_then(|s| s.to_str()) != Some("toy") {
        return Err(anyhow!("File must have .toy extension: {:?}", path));
    }

    // 2. Read source code
    let source = fs::read_to_string(path)
        .with_context(|| format!("Failed to read file: {:?}", path))?;

    // 3. JIT Compile
    let mut jit = jit::JIT::default();
    let code_ptr = jit.compile(&source)
        .map_err(|e| anyhow!("Compilation error: {}", e))?;

    // 4. Execute (assuming no arguments for now, or main)
    // The current toy language JIT.compile returns the entry point of the function defined in the script.
    unsafe {
        let func = mem::transmute::<_, extern "C" fn() -> i64>(code_ptr);
        let result = func();
        println!("Return value: {}", result);
    }

    Ok(())
}

fn run_all_tests() -> Result<()> {
    let mut jit = jit::JIT::default();

    println!("foo(1, 0) = {}", run_foo(&mut jit).map_err(|e| anyhow!(e))?);
    println!("recursive_fib(10) = {}", run_recursive_fib(&mut jit, 10).map_err(|e| anyhow!(e))?);
    println!("iterative_fib(10) = {}", run_iterative_fib(&mut jit, 10).map_err(|e| anyhow!(e))?);
    println!("float_add(1.5, 2.5) = {}", run_float_add(&mut jit, 1.5, 2.5).map_err(|e| anyhow!(e))?);
    println!("mixed_add(10, 2.5) = {}", run_mixed_add(&mut jit, 10, 2.5, 2.5).map_err(|e| anyhow!(e))?);
    
    run_hello(&mut jit).map_err(|e| anyhow!(e))?;
    
    println!("mul_div(10.0, 5.0) = {}", run_mul_div(&mut jit, 10.0, 5.0).map_err(|e| anyhow!(e))?);
    
    run_custom_string(&mut jit, "Customize String Test Success!").map_err(|e| anyhow!(e))?;
    
    println!("--- Running String Literal Test ---");
    run_string_test(&mut jit).map_err(|e| anyhow!(e))?;

    println!("--- Running I128 Test ---");
    run_i128_test(&mut jit).map_err(|e| anyhow!(e))?;

    println!("--- Running Complex Test ---");
    run_complex_test(&mut jit).map_err(|e| anyhow!(e))?;
    
    println!("--- Running Array Test ---");
    run_array_test(&mut jit).map_err(|e| anyhow!(e))?;

    println!("--- Running Dynamic Array Test ---");
    run_dynamic_array_test(&mut jit).map_err(|e| anyhow!(e))?;

    #[cfg(feature = "mkl")]
    {
        println!("--- Running MKL DGEMM Test ---");
        run_mkl_test(&mut jit).map_err(|e| anyhow!(e))?;
    }

    Ok(())
}

// --- Test helper functions (migrated from original toy.rs) ---

fn run_mkl_test(jit: &mut jit::JIT) -> Result<(), String> {
    let code = r#"
    fn test_mkl(c: [f64; 4]) -> (r: i64) {
        a = [1.0, 2.0, 3.0, 4.0]
        b = [5.0, 6.0, 7.0, 8.0]
        toy_mkl_dgemm(2, 2, 2, 1.0, a, 0.0, b, c)
        r = 0
    }
    "#;
    
    unsafe {
        let code_ptr = jit.compile(code)?;
        let func: extern "C" fn(*mut f64) -> i64 = mem::transmute(code_ptr);
        
        let mut c = [0.0f64; 4];
        func(c.as_mut_ptr());
        
        println!("MKL DGEMM Result Matrix C: {:?}", c);
        if c[0] == 19.0 && c[1] == 22.0 && c[2] == 43.0 && c[3] == 50.0 {
            println!("MKL DGEMM Test Passed!");
            Ok(())
        } else {
            Err(format!("MKL DGEMM Test Failed: expected [19, 22, 43, 50], got {:?}", c))
        }
    }
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
        let code_fn = mem::transmute::<_, extern "C" fn() -> i64>(code_ptr);
        let result = code_fn();
        println!("Complex Test Status: {}", result);
        if result == 1 { Ok(result) } else { Err(format!("Complex test failed")) }
    }
}

fn run_array_test(jit: &mut jit::JIT) -> Result<i64, String> {
     unsafe {
        let code_ptr = jit.compile(ARRAY_TEST_CODE)?;
        let code_fn = mem::transmute::<_, extern "C" fn() -> i64>(code_ptr);
        let result = code_fn();
        println!("Array test result: {}", result);
        if result == 30 { Ok(result) } else { Err(format!("Array test failed: expected 30, got {}", result)) }
    }
}

fn run_dynamic_array_test(jit: &mut jit::JIT) -> Result<i64, String> {
     unsafe {
        let code_ptr = jit.compile(DYNAMIC_ARRAY_TEST_CODE)?;
        let code_fn = mem::transmute::<_, extern "C" fn() -> i64>(code_ptr);
        let result = code_fn();
        println!("Dynamic array test result: {}", result);
        if result == 40 { Ok(result) } else { Err(format!("Dynamic array test failed: expected 40, got {}", result)) }
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

const DYNAMIC_ARRAY_TEST_CODE: &str = r#"
fn dynamic_array_test() -> (r: i64) {
    arr = array [10, 20, 30]
    array_push(arr, 40)
    r = arr[3]
}
"#;
