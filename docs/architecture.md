# Cranelift JIT Demo Architecture

## 1. CLI Design and Entry Flow

The project has been upgraded from a hardcoded test runner to a standardized compiler frontend tool using `clap`.

### CLI Commands

- `cargo run -- <FILE>.toy`: Reads the specified `.toy` script, parses it, compiles it via JIT, and executes the resulting function.
- `cargo run -- --test`: Executes the internal integration test suite.
- `cargo run -- --help`: Displays usage information and available options.

### Execution Flow

1. **Argument Parsing**: `src/cli/mod.rs` defines the `Cli` struct using `clap`.
2. **Mode Dispatch**: `src/bin/toy.rs` decides whether to run a script or the test suite.
3. **Script Mode**:
   - Validate file existence and `.toy` extension.
   - Read source to string.
   - Initialize `JIT` context.
   - `jit.compile(source)` -> AST -> Cranelift IR -> Machine Code Pointer.
   - Execute the machine code pointer via `mem::transmute`.
4. **Test Mode**:
   - Sequentially runs a set of predefined test cases (e.g., Fibonacci, String literals, MKL).
   - Each test uses a fresh JIT context or reuses one as appropriate.

## 2. Runtime Registry and Decoupling

The runtime system is modularized to separate JIT logic from symbol registration.

### Modular Structure

- `src/runtime/mod.rs`: Main entry point for the runtime system.
- `src/runtime/registry.rs`: Centralizes "declarative" registration of built-in functions.
- `src/runtime/io.rs`: Standard IO and misc runtime functions (`putchar`, `rand`).
- `src/runtime/math.rs`: Mathematical functions (`sin`, `cos`, etc.).
- `src/runtime/string.rs`: String-related utilities and `libc` wrappers (`printf`, `puts`).
- `src/runtime/mkl.rs`: Intel MKL integration (feature-gated).

### Registration Mechanism

- **`runtime_fn!` Macro**: A standardized way to define runtime functions with C ABI and no mangling.
- **`register_builtins(builder)`**: Decouples `jit.rs` from individual symbol calls. `jit.rs` now only calls this single function, reducing boilerplate.

## 3. Feature Flags and Configuration

Compile-time features control the availability of advanced runtime capabilities:

- `mkl`: Links with `intel-mkl-src` and registers `toy_mkl_dgemm`.
- `gpu`: (Future) Placeholder for CUDA/GPU interface registration.
- **Default**: Minimal set containing basic arithmetic, IO, and math functions.

### Linkage Safety

If a script calls an MKL function in a build where the `mkl` feature is disabled, the JIT linker will return a `LinkError` (symbol not found) instead of a segmentation fault, as the symbol was never registered in the `JITBuilder`.

## 4. Performance Benchmarks (Target)

- **Cold Start**: Script cold start (from `cargo run` to execution) is targeted at ≤ 100 ms (excluding cargo overhead).
- **Regression Tests**: Total duration for integration tests should remain efficient with minimal overhead from the new CLI architecture.

## 5. Backward Compatibility

- The old hardcoded test mode is still available via `cargo run -- --test`.
- **Deprecated**: Direct use of `jit.compile` with hardcoded strings in `main.rs` is deprecated in favor of the CLI-based script execution.
- **Migration**: To migrate old tests, wrap them into `.toy` files and run them using the new CLI or add them to the `run_all_tests` suite in `src/bin/toy.rs`.
