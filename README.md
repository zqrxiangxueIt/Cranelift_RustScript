
# RustScript JIT Engine

## Overview

This project transforms the `cranelift-jit-demo` into a production-grade JIT compiler for "RustScript", a scripting language utilizing Rust syntax. It leverages `syn` for robust parsing and `cranelift` for high-performance machine code generation.

## Architecture

### Frontend

* **Parser:** Uses the `syn` crate to parse standard Rust syntax, replacing the legacy `peg` implementation.
* **AST Analysis:** Converts `syn` AST nodes into a custom intermediate representation optimized for JIT compilation.

### Core Compiler

* **SSA Construction:** Uses `cranelift-frontend` to handle SSA (Static Single Assignment) variable renaming and Phi node insertion automatically.
* **Type System:** Implements a `TypeRegistry` to handle struct memory layouts, alignment, and field offsets manually, mapping high-level types to raw pointer arithmetic.
* **Backend:** `cranelift-jit` manages memory executable permissions, symbol resolution, and function relocation.

## Features

* **Rust Syntax:** Native support for `let mut` bindings, block scopes, and expressions.
* **Composite Types:** Support for defining and using structs with C-compatible memory layout.
* **Control Flow:** Implementation of `if-else` expressions and `while` loops with correct basic block sealing.
* **Host Interop:** Mechanism to register and call host Rust functions from within the script.

## Getting Started

### Prerequisites

* Rust (Stable)
* Cargo

### Build and Run

```bash
git clone https://github.com/your-username/rustscript-jit
cd rustscript-jit
cargo run --release
```

### Example Script

```rust
struct Point {
    x: i32,
    y: i32,
}

fn main() -> i32 {
    let mut p = Point { x: 10, y: 20 };
    p.x = p.x + p.y;
    p.x
}
```

## License

Apache-2.0