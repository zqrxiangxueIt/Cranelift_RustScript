// src/main.rs
mod ast;
mod frontend;
mod grammar;
mod jit;

use std::io::{self, Write};

fn main() {
    let mut jit = jit::JIT::default();
    println!("=== RustScript JIT Compiler v0.1 ===");
    println!("输入表达式并回车执行 (例如: a = 5; a * 10)");
    println!("输入空行退出。");

    loop {
        print!(">> ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).unwrap() == 0 {
            break;
        }

        let trimmed = input.trim();
        if trimmed.is_empty() {
            break;
        }

        // 解析 -> 编译 -> 运行
        match grammar::rustscript_parser::program(trimmed) {
            Ok(stmts) => {
                match jit.compile_and_run(stmts) {
                    Ok(result) => println!("Result: {}", result),
                    Err(e) => println!("JIT Error: {}", e),
                }
            }
            Err(e) => println!("Parse Error: {}", e),
        }
    }
}