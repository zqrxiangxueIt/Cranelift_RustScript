I will implement the requested type system extension for the Cranelift JIT Demo. This involves a comprehensive update to the frontend parser, AST, and JIT compilation logic to support multiple integer widths (`i8`-`i128`) and floating-point types (`f32`, `f64`), along with type conversion rules.

### 1. Frontend & AST Extensions (`src/frontend.rs`)

* **Define Type System**: Introduce a `Type` enum supporting `I8`, `I16`, `I32`, `I64`, `I128`, `F32`, `F64`.

* **Update AST (`Expr`)**:

  * Replace generic `Literal` with typed `IntLiteral(i128, Type)` and `FloatLiteral(f64, Type)`.

  * Add `Cast(Box<Expr>, Type)` for explicit type conversions.

* **Enhance Parser (PEG grammar)**:

  * **Typed Signatures**: Update function definitions to require types: `fn name(arg: type, ...) -> (ret: type)`.

  * **Literals**: Parse integer and floating-point literals with type inference (e.g., `1.0` as `f64`, `1` as `i64` default).

  * **Explicit Casts**: Add syntax `expr as type`.

### 2. Type Analysis & Variable Declaration

* **Type Inference Pass**: Implement a pre-translation pass to determine the type of every variable.

  * Variables declared in function arguments use the explicit type.

  * Local variables infer their type from the first assignment.

  * Subsequent assignments must match the inferred type or be explicitly cast.

* **Symbol Table**: Update variable declaration logic to map names to specific Cranelift types, ensuring correct stack slot allocation.

### 3. JIT Compilation Logic (`src/jit.rs`)

* **Update** **`FunctionTranslator`**:

  * **Type Awareness**: Track the type of each variable and intermediate value.

  * **Arithmetic & Logic**: Implement `iadd`, `fadd`, `imul`, `fmul`, etc., dispatching based on operand types.

  * **Comparisons**: Implement `icmp` for integers and `fcmp` for floats.

  * **Conversions**:

    * **Implicit**: Implement automatic widening (e.g., `i32` -> `i64`) in binary operations.

    * **Explicit**: Implement `translate_cast` using Cranelift's `uextend`, `sextend`, `fcvt`, etc.

* **IR Generation**: Ensure the correct Cranelift IR types are generated for all operations.

### 4. Testing & Documentation

* **Update** **`src/bin/toy.rs`**:

  * Refactor existing tests to use typed function signatures.

  * Add new test cases for:

    * Small integers (`i8`, `i16`).

    * Floating point arithmetic (`f32`, `f64`).

    * Mixed-type operations demonstrating implicit conversion.

    * Explicit casting.

* **Documentation**: Update `README.md` with the new language syntax and type system details.

### 5. Verification

* Run the updated `toy.rs` to verify correct execution of all new types and operations.

