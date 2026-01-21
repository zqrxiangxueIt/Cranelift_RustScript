use core::mem;
use cranelift_jit_demo::jit;

fn main() -> Result<(), String> {
    // 程序主函数，实现toy语言的解释执行
    let mut jit = jit::JIT::default();
    
    println!("foo(1, 0) = {}", run_foo(&mut jit)?);    // 调用foo函数，参数为1和0，返回值为30
    
    println!(
        "recursive_fib(10) = {}",
        run_recursive_fib(&mut jit, 10)?    // 调用recursive_fib函数，参数为10，返回值为55
    );
    
    println!(
        "iterative_fib(10) = {}",
        run_iterative_fib(&mut jit, 10)?    // 调用iterative_fib函数，参数为10，返回值为55
    );
    
    println!(
        "float_add(1.5, 2.5) = {}", 
        run_float_add(&mut jit, 1.5, 2.5)?    // 调用float_add函数，参数为1.5和2.5，返回值为4.0
    );
    
    println!(
        "mixed_add(10, 2.5) = {}", 
        run_mixed_add(&mut jit, 10, 2.5)?    // 调用mixed_add函数，参数为10和2.5，返回值为12.5
    );
    
    run_hello(&mut jit)?;
    Ok(())
}

fn run_foo(jit: &mut jit::JIT) -> Result<i64, String> {     //输入参数为i64类型的a和b，返回值为i64类型的c
    unsafe {
        let code_ptr = jit.compile(FOO_CODE)?;     // 编译FOO_CODE代码，返回函数指针code_ptr
        let code_fn = mem::transmute::<_, extern "C" fn(i64, i64) -> i64>(code_ptr);     // 将函数指针转换为extern "C" fn(i64, i64) -> i64类型的函数指针
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

fn run_mixed_add(jit: &mut jit::JIT, a: i32, b: f64) -> Result<f64, String> {
    unsafe {
        let code_ptr = jit.compile(MIXED_ADD_CODE)?;
        let code_fn = mem::transmute::<_, extern "C" fn(i32, f64) -> f64>(code_ptr);
        Ok(code_fn(a, b))
    }
}

fn run_hello(jit: &mut jit::JIT) -> Result<i64, String> {
    jit.create_data("hello_string", "hello world!\0".as_bytes().to_vec())?;   //在内存中开辟一块区域 ，存放我们想要用的字符串数据
    unsafe {
        let code_ptr = jit.compile(HELLO_CODE)?;
        let code_fn = mem::transmute::<_, extern "C" fn() -> i64>(code_ptr);
        Ok(code_fn())
    }
}
// FOO_CODE: 实现一个简单的函数foo，根据输入参数a和b返回不同的结果，同时对结果进行加2操作
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
// RECURSIVE_FIB_CODE: 实现一个递归函数recursive_fib，计算斐波那契数列的第n项
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
// ITERATIVE_FIB_CODE: 实现一个迭代函数iterative_fib，计算斐波那契数列的第n项
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
// FLOAT_ADD_CODE: 实现一个简单的函数float_add，对输入参数a和b进行浮点数加法操作
const FLOAT_ADD_CODE: &str = r#"
    fn float_add(a: f64, b: f64) -> (c: f64) {
        c = a + b
    }
"#;
// MIXED_ADD_CODE: 实现一个简单的函数mixed_add，对输入参数a和b进行混合类型加法操作，先将a转换为f64类型
const MIXED_ADD_CODE: &str = r#"
    fn mixed_add(a: i32, b: f64) -> (c: f64) {
        c = (a as f64) + b
    }
"#;

const HELLO_CODE: &str = r#"
fn hello() -> (r: i64) {
    puts(&hello_string)
}
"#;
