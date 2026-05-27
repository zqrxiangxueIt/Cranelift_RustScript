# Cranelift JIT Demo — 项目工作原理完整指南

## 目录

- [第〇章：快速定位](#第〇章快速定位)
- [第一章：项目架构与文件地图](#第一章项目架构与文件地图)
  - [1.1 项目简介](#11-项目简介)
  - [1.2 用户如何使用](#12-用户如何使用)
  - [1.3 文件架构图](#13-文件架构图)
  - [1.4 新增功能修改清单](#14-新增功能修改清单)
- [第二章：功能实现详解](#第二章功能实现详解)
  - [2.1 简单运算与类型系统](#21-简单运算与类型系统)
  - [2.2 控制流](#22-控制流)
  - [2.3 字符串与 IO](#23-字符串与-io)
  - [2.4 固定数组](#24-固定数组)
  - [2.5 动态数组与内存管理（重点）](#25-动态数组与内存管理重点)
  - [2.6 复数运算](#26-复数运算)
  - [2.7 外部函数调用与 MKL 集成](#27-外部函数调用与-mkl-集成)
- [第三章：完整编译器流水线](#第三章完整编译器流水线)
  - [3.1 入口 — `src/bin/toy.rs`](#31-入口--srcbintoyrs)
  - [3.2 解析 — `src/frontend.rs`](#32-解析--srcfrontendrs)
  - [3.3 常量折叠 — `src/optimizer.rs`](#33-常量折叠--srcoptimizerrs)
  - [3.4 所有权检查 — `src/ownership.rs`](#34-所有权检查--srcownershiprs)
  - [3.5 类型检查器 — `src/type_checker.rs`](#35-类型检查器--srctype_checkerrs)
  - [3.6 JIT 编译器 — `src/jit.rs`](#36-jit-编译器--srcjitrs)
  - [3.7 运行时层 — `src/runtime/`](#37-运行时层--srcruntime)
  - [3.8 执行 — `mem::transmute`](#38-执行--memtransmute)
- [第四章：端到端完整追踪](#第四章端到端完整追踪)
- [第五章：附录](#第五章附录)

---

## 第〇章：快速定位

如果你只想**快速了解**某个方面，可以直接跳转：

| 你想知道... | 跳转到 |
|---|---|
| 这个项目是干什么的？ | [1.1 项目简介](#11-项目简介) |
| 怎么运行它？ | [1.2 用户如何使用](#12-用户如何使用) |
| 如果要加新功能改哪个文件？ | [1.4 新增功能修改清单](#14-新增功能修改清单) |
| 某个功能是怎么实现的？（运算/数组/内存...） | [第二章：功能实现详解](#第二章功能实现详解)（按功能划分，含内存回收时间线） |
| 编译器从源码到执行的完整流程？ | [第三章：完整编译器流水线](#第三章完整编译器流水线)（按流水线阶段阅读） |
| 跟着一个例子走通全流程？ | [第四章：端到端完整追踪](#第四章端到端完整追踪) |
| 某个 AST 节点长什么样？ | [附录 C：AST 节点速查表](#附录-cast-节点速查表) |
| 运行时函数有哪些？ | [附录 B：运行时函数速查表](#附录-b运行时函数速查表) |
| 类型怎么映射到 Cranelift？ | [附录 A：类型映射表](#附录-a类型映射表) |

---

## 第一章：项目架构与文件地图

### 1.1 项目简介

这是一个**基于 Cranelift JIT 的 Toy 语言编译器**，fork 自 [bytecodealliance/cranelift-jit-demo](https://github.com/bytecodealliance/cranelift-jit-demo)。

原始项目只支持 `i64` 类型的加减乘除和 `puts` 调用，经过大量扩展后，当前支持：

- **类型系统**：`i8`/`i16`/`i32`/`i64`/`i128`, `f32`/`f64`, `String`, `Complex64`/`Complex128`, 固定数组 `[T; N]`, 动态数组 `array<T>`
- **语法**：算术/比较运算、`if-else`、`while`、函数调用、数组索引、类型转换 `as`、`drop()` 显式释放
- **运行时**：数学函数 (`sin`/`cos`/`sqrt`/`pow` 等)、IO 函数 (`printf`/`puts`/`print_f64`)、动态数组方法 (`array_push`/`array_pop`/`array_len` 等)
- **编译优化**：常量折叠（代数恒等式消除）
- **静态检查**：编译期所有权检查（防止 DynamicArray 泄漏和 double drop）
- **可选特性**：Intel MKL DGEMM 矩阵乘法（通过 `--features mkl` 启用）

**核心依赖**：

| 库 | 用途 |
|---|---|
| `peg` 0.8.1 | PEG 解析器生成器，定义 Toy 语言语法 |
| `cranelift` 0.125.3 | Cranelift 代码生成核心 |
| `cranelift-jit` 0.125.3 | JIT 后端，内存中编译执行 |
| `cranelift-module` 0.125.3 | 模块抽象，管理函数和数据对象 |
| `cranelift-native` 0.125.3 | 自动检测宿主机器 ISA |
| `libc` 0.2.180 | C 库 FFI（`printf`/`puts`/`c_double`） |
| `clap` 4.5 | CLI 参数解析 |
| `raii_demo` (本地) | RAII 动态数组实现 |
| `intel-mkl-src` 0.8.1 (可选) | Intel MKL BLAS 库 |

### 1.2 用户如何使用

```bash
# 运行内置集成测试（推荐入门方式）
cargo run -- --test

# 运行一个 .toy 脚本文件
cargo run -- examples/array_basic.toy

# 启用 MKL 特性运行测试
cargo run --features mkl -- --test

# 运行单元测试
cargo test

# 运行基准测试
cargo bench
```

**示例 `.toy` 脚本**（`examples/` 目录下有 6 个）：

```toy
# examples/array_iteration.toy — 动态数组求和
fn main() -> (sum: i64) {
    arr = array [1, 2, 3, 4, 5]
    sum = 0
    i = 0
    len = array_len(arr)
    while i < len {
        sum = sum + arr[i]
        i = i + 1
    }
}
```

### 1.3 文件架构图

**编译流水线**（核心数据流）：

```
                  ┌──────────────┐
  用户输入 ──────>│  src/bin/    │  CLI 入口，选择 test 或 script 模式
  .toy 文件       │  toy.rs      │
                  └──────┬───────┘
                         │ jit.compile(&source)
                         ▼
                  ┌──────────────┐
                  │  frontend.rs │  PEG 语法解析 → AST (Vec<Expr>)
                  └──────┬───────┘
                         │ stmts
                         ▼
                  ┌──────────────┐
                  │ optimizer.rs │  常量折叠优化 → 简化后的 AST
                  └──────┬───────┘
                         │ stmts (optimized)
                         ▼
                  ┌──────────────┐
                  │ ownership.rs │  所有权检查 → 拒绝泄漏/double drop
                  └──────┬───────┘
                         │ stmts (validated)
                         ▼
           ┌────────────────────────────┐
           │         jit.rs             │
           │  ┌──────────────────────┐  │
           │  │ type_checker.rs      │  │  类型推导 + 签名查询
           │  │ (infer_type,         │  │
           │  │  resolve_func)       │  │
           │  └──────────────────────┘  │
           │  FunctionTranslator        │  AST → Cranelift IR
           │  ↓                         │
           │  JITModule                 │  IR → 机器码
           └────────────┬───────────────┘
                        │ code ptr
                        ▼
                 ┌──────────────┐
                 │ mem::transmute│  函数指针转换 + 调用
                 └──────────────┘
                        │
          ┌─────────────┼─────────────┐
          ▼             ▼             ▼
    ┌──────────┐ ┌──────────┐ ┌──────────┐
    │ runtime/ │ │ runtime/ │ │ runtime/ │
    │ array.rs │ │ math.rs  │ │ io.rs    │  extern "C" 函数
    │          │ │          │ │          │  被 JIT 代码调用
    └──────────┘ └──────────┘ └──────────┘
          ▲
          │ 使用
    ┌──────────────┐
    │ raii_demo/   │
    │ lib.rs       │  DynamicArray<T> RAII 实现
    └──────────────┘
```

**源文件模块地图**：

| 文件 | 关键类型/函数 | 职责 |
|---|---|---|
| `src/bin/toy.rs` | `main()`, `run_script()`, `run_all_tests()` | CLI 入口，内置测试集 |
| `src/cli/mod.rs` | `Cli` (clap struct) | 命令行参数定义 |
| `src/lib.rs` | `mod` 声明 | crate 根，模块树 |
| `src/frontend.rs` | `Expr`, `Type`, `parser::function()` | AST 定义 + PEG 语法 |
| `src/optimizer.rs` | `fold_constants()`, `fold_constants_in_stmts()` | 常量折叠优化 |
| `src/ownership.rs` | `OwnershipChecker`, `ArrayDisposition`, `OwnershipError` | DynamicArray 所有权检查 |
| `src/type_checker.rs` | `TypeChecker`, `FunctionSignature`, `infer_type()` | 类型推导 + 内置函数签名 |
| **`src/jit.rs`** | `JIT`, `FunctionTranslator`, `compile()`, `translate()` | **核心**：JIT 编译全流程 |
| `src/runtime/mod.rs` | `mod` 声明 | 运行时模块根 |
| `src/runtime/registry.rs` | `register_builtins()`, `runtime_fn!` 宏 | 注册所有 extern C 函数到 JIT |
| `src/runtime/array.rs` | `dynamic_array_*_i64/f64/complex128` | 动态数组 C ABI 函数 |
| `src/runtime/math.rs` | `toy_sin/cos/tan/sqrt/exp/log/ceil/floor/pow` | 数学函数 |
| `src/runtime/io.rs` | `toy_putchar/rand/sum_array/print_f64/print_i64` | IO 函数 |
| `src/runtime/string.rs` | re-export `libc::printf`, `libc::puts` | 字符串输出 |
| `src/runtime/mkl.rs` | `cblas_dgemm` FFI, `toy_mkl_dgemm` | MKL 矩阵乘法（条件编译） |
| `raii_demo/src/lib.rs` | `DynamicArray<T>` | RAII 动态数组（被 runtime/array.rs 使用） |
| `tests/integration_test.rs` | 集成测试 | sin/pow/sdiv/MKL 集成测试 |
| `tests/type_checker_test.rs` | 类型检查测试 | `resolve_func`, `infer_type` 测试 |
| `benches/jit_bench.rs` | 基准测试 | JIT vs Native 性能对比 |

### 1.4 新增功能修改清单

| 需求 | 需要修改的文件 |
|---|---|
| 新增一种**语法**（如 for 循环） | `frontend.rs` — `Expr` 加变体 + PEG 加 rule + `jit.rs` — `translate_expr` 加分支 + `type_checker.rs` — `infer_type` 加分支 + `optimizer.rs` — `fold_constants` 加分支（可选） + `ownership.rs` — `analyze_expr` 加分支（如果涉及 DynamicArray） |
| 新增一种**字面量类型**（如 bool） | `frontend.rs` — `Type` 加变体 + `Expr::Literal` 扩展 + PEG `literal()` + `jit.rs` — `to_cranelift_type()` + `translate_expr` 的 `Literal` 分支 + `type_checker.rs` — `infer_type` |
| 新增一个**内置函数**（如 `toy_foo`） | `runtime/io.rs` 或新文件 — 实现 `extern "C" fn` + `runtime/registry.rs` — 注册 `builder.symbol(...)` + `type_checker.rs` — `register_builtins()` 加签名 + `jit.rs` — 如需特殊参数展开则修改 `translate_call` |
| 新增一个**优化 pass**（如死代码消除） | 新建 `src/dead_code.rs` + `src/lib.rs` — 加 `pub mod dead_code` + `src/jit.rs` `compile()` — 插入调用 |
| 新增一个**静态检查**（如类型不匹配警告） | 新建 `src/my_check.rs` + `src/lib.rs` — 加 mod + `src/jit.rs` `compile()` — 插入调用 |
| DynamicArray 支持**新元素类型**（如 String） | `runtime/array.rs` — 加函数族 + `runtime/registry.rs` — 注册 + `type_checker.rs` — 加签名 + `jit.rs` — `translate_dynamic_array_literal` 和 `translate_index` 加分支 |

---

## 第二章：功能实现详解
---

### 2.1 简单运算与类型系统

**涉及文件**：`frontend.rs` (Expr/Type 枚举) → `type_checker.rs` (`infer_type`) → `jit.rs` (`translate_binary_op`, `translate_cmp`, `translate_cast`, `promote_operands`)

**依赖库**：`cranelift` 提供 I8-I128 / F32 / F64 IR 类型和对应的整数/浮点指令

#### 核心代码路径

**二元运算** — `src/jit.rs:326-393`：

当 `translate_expr` 遇到 `Expr::Add(lhs, rhs)` 时，先通过 `infer_type` 判断操作数类型。如果不是复数，走 `translate_binary_op`（`jit.rs:446-455`）：

```rust
fn translate_binary_op<F>(&mut self, lhs: Expr, rhs: Expr, op: F) -> Value
where
    F: Fn(&mut FunctionBuilder, Value, Value) -> Value,
{
    let l_val = self.translate_expr(lhs);
    let r_val = self.translate_expr(rhs);
    let (l_promoted, r_promoted) = self.promote_operands(l_val, r_val);
    op(&mut self.builder, l_promoted, r_promoted)
}
```

关键步骤 `promote_operands`（`jit.rs:457-489`）实现了**隐式类型提升**：

- `i32 + i64` → 将 `i32` 通过 `sextend` 符号扩展为 `i64`，然后 `i64 + i64`
- `f32 + f64` → 将 `f32` 通过 `fpromote` 提升为 `f64`，然后 `f64 + f64`
- 不允许 int ↔ float 之间的隐式转换（必须显式 `as`）

实际指令选择在闭包中（如 Add 的闭包，`jit.rs:333-339`）：

```rust
|b, l, r| {
    let ty = b.func.dfg.value_type(l);
    if ty.is_float() { b.ins().fadd(l, r) } else { b.ins().iadd(l, r) }
}
```

**比较运算** — `src/jit.rs:528-543`：

比较结果统一为 `I64`（Toy 语言没有 bool 类型，用 `0` 和 `1` 表示）：

```rust
fn translate_cmp(&mut self, lhs: Expr, rhs: Expr, int_cc: IntCC, float_cc: FloatCC) -> Value {
    let (l, r) = self.promote_operands(l_val, r_val);
    let bool_res = if ty.is_float() {
        self.builder.ins().fcmp(float_cc, l, r)  // 浮点比较
    } else {
        self.builder.ins().icmp(int_cc, l, r)    // 整数比较
    };
    // bool_res 是 Cranelift 的布尔值，用 select 转为 I64 的 1 或 0
    let one = InstBuilder::iconst(self.builder.ins(), types::I64, 1);
    let zero = InstBuilder::iconst(self.builder.ins(), types::I64, 0);
    self.builder.ins().select(bool_res, one, zero)
}
```

**类型转换 (`as`)** — `src/jit.rs:492-525`：

支持四种转换路线：

| 转换方向 | Cranelift 指令 |
|---|---|
| int → 更宽的 int | `sextend` (符号扩展) |
| int → 更窄的 int | `ireduce` (截断) |
| float → 更宽的 float | `fpromote` |
| float → 更窄的 float | `fdemote` |
| int → float | `fcvt_from_sint` |
| float → int | `fcvt_to_sint` |

#### 示例脚本 Walkthrough

`MIXED_ADD_CODE`（`src/bin/toy.rs:331-335`）：

```toy
fn mixed_add(a: i32, b: f64, c: f64) -> (r: f64) {
    r = (a as f64) + b + c
}
```

编译过程：
1. `a as f64` → `translate_cast` 发射 `fcvt_from_sint(F64, a_val)` — 将 `i32` 转为 `f64`
2. `(a as f64) + b` → `translate_binary_op` → 两边都是 `F64`，不需要 promote → `ins().fadd`
3. `...+ c` → 同样 `ins().fadd`
4. 整个表达式结果是 `F64`

#### 设计优势

Cranelift 原生支持这些类型和操作，JIT 生成的机器码与 AOT 编译的 Rust/C 代码在**指令级别完全等价**——没有解释器开销，没有运行时类型检查。

---

### 2.2 控制流

**涉及文件**：`frontend.rs` (IfElse/WhileLoop AST 节点 + PEG rule) → `jit.rs` (`translate_if_else`, `translate_while_loop`)

**依赖库**：`cranelift` 提供 block / brif / jump / block_params 基本块控制流原语

#### if-else 实现 — `src/jit.rs:612-664`

Cranelift IR 使用基本块 + block params 实现 phi 节点。`translate_if_else` 创建三个块：

```
entry_block
  ├── brif condition → then_block / else_block
  │
then_block                      else_block
  ├── 执行 then_body             ├── 执行 else_body
  └── jump merge_block(v_then)   └── jump merge_block(v_else)
                                  │
                          merge_block(v_result)
                            └── 使用 v_result
```

核心代码：

```rust
// 1. 创建三个块
let then_block = self.builder.create_block();
let else_block = self.builder.create_block();
let merge_block = self.builder.create_block();

// 2. 条件跳转
self.builder.ins().brif(condition_value, then_block, &[], else_block, &[]);

// 3. 翻译 then 分支，获取最后结果
self.builder.switch_to_block(then_block);
let mut then_return = ...; // 默认值
for expr in then_body { then_return = self.translate_expr(expr); }
// 在 merge_block 上创建 block param 接收 then 的值
let then_ty = self.builder.func.dfg.value_type(then_return);
self.builder.append_block_param(merge_block, then_ty);
self.builder.ins().jump(merge_block, &[BlockArg::Value(then_return)]);

// 4. 同样处理 else 分支
// 5. switch_to_block(merge_block)，使用 block_params(merge_block)[0] 作为结果
```

#### while 循环实现 — `src/jit.rs:667-693`

```
entry_block → jump →
header_block              body_block
  ├── 求值 condition       ├── 执行 loop_body
  └── brif → body/exit     └── jump → header_block
                              │
                      exit_block (循环结束)
```

```rust
let header_block = self.builder.create_block();
let body_block = self.builder.create_block();
let exit_block = self.builder.create_block();

self.builder.ins().jump(header_block, &[]);
self.builder.switch_to_block(header_block);

let condition_value = self.translate_expr(condition);  // 每次循环重新求值
self.builder.ins().brif(condition_value, body_block, &[], exit_block, &[]);

self.builder.switch_to_block(body_block);
for expr in loop_body { self.translate_expr(expr); }
self.builder.ins().jump(header_block, &[]);  // 跳回 header 形成循环

self.builder.switch_to_block(exit_block);
```

#### 示例脚本 Walkthrough

`ITERATIVE_FIB_CODE`（`src/bin/toy.rs:307-323`）：

```toy
fn iterative_fib(n: i64) -> (r: i64) {
    if n == 0 { r = 0 }
    else {
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
```

控制流结构：`if` → `else` → `while`，编译为原生的条件跳转和循环跳转指令，运行时**零抽象开销**。

#### 设计优势

完全编译为原生 CPU 跳转指令（`jmp`/`je`/`jne` 等），与 C 的 `if`/`while` 生成的机器码无区别。while 循环不需要任何 GC 安全点或栈展开检查。

---

### 2.3 字符串与 IO

**涉及文件**：`frontend.rs` (StringLiteral, GlobalDataAddr) → `jit.rs` (`translate_string_literal`, `translate_global_data_addr`, `translate_call`) → `runtime/string.rs` → `runtime/io.rs`

**依赖库**：`libc::printf`、`libc::puts`（直接复用 C 标准库）

#### 字符串字面量 — `src/jit.rs:773-793`

Toy 语言中的 `"hello"` 在编译时被写入 JIT 模块的数据段，作为 null-terminated C 字符串存储：

```rust
fn translate_string_literal(&mut self, s: String) -> Value {
    self.string_counter += 1;
    let name = format!("str_{}_{}", self.current_func_name, self.string_counter);

    let mut data_ctx = DataDescription::new();
    let mut bytes = s.as_bytes().to_vec();
    bytes.push(0);  // ← null 终止符，兼容 printf/puts
    data_ctx.define(bytes.into_boxed_slice());

    // 声明并定义全局数据
    let data_id = self.module.declare_data(&name, Linkage::Local, false, false).unwrap();
    self.module.define_data(data_id, &data_ctx).unwrap();

    // 获取数据的地址，作为 I64 值返回
    let local_id = self.module.declare_data_in_func(data_id, self.builder.func);
    let pointer = self.module.target_config().pointer_type();
    self.builder.ins().symbol_value(pointer, local_id)
}
```

之后 `puts(s)` 调用时，`s` 的值就是这个 I64 指针，直接传给 `libc::puts`。

#### 外部数据引用 — `src/jit.rs:762-771`

Toy 支持 `&name` 语法引用 Rust 侧预注册的全局数据（如 `hello_string`）：

```rust
fn translate_global_data_addr(&mut self, name: String) -> Value {
    let sym = self.module.declare_data(&name, Linkage::Export, true, false).unwrap();
    let local_id = self.module.declare_data_in_func(sym, self.builder.func);
    let pointer = self.module.target_config().pointer_type();
    self.builder.ins().symbol_value(pointer, local_id)
}
```

Rust 侧通过 `jit.create_data("hello_string", ...)` 注册数据，Toy 代码中 `&hello_string` 获取其地址。

#### IO 函数

`toy_putchar` / `toy_rand` / `toy_print_f64` / `toy_print_i64`（`src/runtime/io.rs`）是简单的 `extern "C"` 包装函数：

```rust
pub extern "C" fn toy_print_f64(n: f64) -> f64 { println!("{}", n); n }
pub extern "C" fn toy_print_i64(n: i64) -> i64 { println!("{}", n); n }
```

#### 示例脚本 Walkthrough

`STRING_TEST_CODE`（`src/bin/toy.rs:355-365`）：

```toy
fn string_test() -> (r: i64) {
    s = "Hello from JIT String Literal!\nWith Newline\tAnd Tab"
    puts(s)
    fmt = "Printf Test: %s %d\n"
    world = "World"
    num = 123
    printf(fmt, world, num)
    r = 0
}
```

执行时的数据流动：
1. `"Hello from..."` → `translate_string_literal` 在模块中创建数据段（含 `\0`）→ 获得 I64 指针
2. `puts(s)` → `translate_call("puts", [I64指针])` → `call libc::puts`
3. `printf(fmt, world, num)` → 三个参数都是 I64 指针或值 → `call libc::printf`

**内存回收**：字符串数据段在 `JITModule` 中分配，生命周期等同于 JIT 实例。`puts`/`printf` 内部使用 C 标准库的缓冲区，无额外回收需求。

#### 设计优势

直接复用 C 标准库的 `printf`/`puts`，Toy 语言不需要自己实现格式化输出引擎。字符串字面量自动转为 null-terminated C 字符串，与 `printf` 的 `%s` 格式符完全兼容。

---

### 2.4 固定数组

**涉及文件**：`frontend.rs` (ArrayLiteral, Index, Array(T,N) type) → `jit.rs` (`translate_array_literal`, `translate_index` 固定数组路径) → `type_checker.rs` (`infer_type` Array)

**依赖库**：无额外依赖，纯 Cranelift IR 实现

#### 创建 — `src/jit.rs:823-859`

固定数组 `[elem1, elem2, ...]` 在**栈上**分配内存：

```rust
fn translate_array_literal(&mut self, elems: Vec<Expr>, _ty: FrontendType) -> Value {
    // 1. 推断元素类型和数组长度
    let elem_ty = type_checker::infer_type(&elems[0], ...);
    let len = elems.len();
    let cl_elem_ty = to_cranelift_type(&elem_ty);
    let elem_size = cl_elem_ty.bytes();
    let total_size = elem_size * (len as u32);

    // 2. 在栈上分配 elem_size × len 字节的栈槽
    let slot = self.builder.create_sized_stack_slot(StackSlotData {
        kind: StackSlotKind::ExplicitSlot,
        size: total_size,
        align_shift: (elem_size as f64).log2().ceil() as u8,
    });

    // 3. 逐个元素写入
    for (i, elem) in elems.into_iter().enumerate() {
        let val = self.translate_expr(elem);
        let offset = (i as i32) * (elem_size as i32);
        self.builder.ins().stack_store(val, slot, offset);
    }

    // 4. 返回栈上数组的基址指针
    self.builder.ins().stack_addr(types::I64, slot, 0)
}
```

#### 索引与边界检查 — `src/jit.rs:972-995`

固定数组的索引走 `is_dynamic = false` 路径：

```rust
// 1. 计算元素地址
let elem_size = cl_elem_ty.bytes() as i64;
let offset = self.builder.ins().imul_imm(idx_val_i64, elem_size);
let addr = self.builder.ins().iadd(base_val, offset);

// 2. 编译期插入边界检查
if len > 0 {
    let len_val = self.builder.ins().iconst(types::I64, len as i64);
    let out_of_bounds = self.builder.ins().icmp(
        IntCC::UnsignedGreaterThanOrEqual, idx_val_i64, len_val);
    self.builder.ins().trapnz(out_of_bounds, TrapCode::unwrap_user(1));
}

// 3. 从地址加载元素
self.builder.ins().load(cl_elem_ty, MemFlags::new(), addr, 0)
```

`trapnz` 的作用：当 `idx >= len` 时，CPU 触发硬件 trap，程序终止 —— 不会发生内存越界读写的未定义行为。

#### 内存回收

**栈分配，函数返回时自动释放。** 不需要任何显式的 `drop` 或 GC。Cranelift 生成的函数序言/尾声自动管理栈帧大小。

#### 外部函数调用中的数组展开 — `src/jit.rs:709-735`

当固定数组作为参数传给外部 C 函数时，Toy 编译器自动将 `[T; N]` 展开为 `(ptr, len)` 两个参数：

```rust
let should_expand = if let FrontendType::Array(_, _) = arg_ty {
    signature.map(|s| s.is_external).unwrap_or(false)
} else { false };

if should_expand {
    arg_values.push(val);              // 指针
    sig.params.push(AbiParam::new(...));
    arg_values.push(len_val);          // 长度
    sig.params.push(AbiParam::new(types::I64));
}
```

这使得 Toy 的 `[f64; 4]` 可以直接传给 `toy_sum_array(*const f64, usize)` 这样的 C 函数。

#### 示例脚本 Walkthrough

`ARRAY_TEST_CODE`（`src/bin/toy.rs:376-382`）：

```toy
fn array_test() -> (r: i64) {
    arr = [10, 20, 30]   // 栈上分配 3×8=24 字节
    x = arr[2]            // 编译时已知 idx=2 < len=3，边界检查通过
    r = x                 // 结果 30
}                         // 函数返回，栈帧自动释放
```

#### 设计优势

- **零分配开销**：栈分配，无需调用 allocator
- **零额外安全检查**：边界检查编译进机器码（一条 `cmp` + 一条条件跳转/trap），无运行时函数调用
- **与 C 数组 ABI 兼容**：自动展开为 `(ptr, len)` 传给 C 函数

---

### 2.5 动态数组与内存管理（重点）

这是项目中最复杂的子系统，涉及四层架构：

```
┌─────────────────────────────────────────┐
│  编译期检查层: ownership.rs              │
│  状态机: Owned→Returned/Dropped/Passed  │
│  防止: 泄漏、double drop、use-after-drop │
└──────────────────┬──────────────────────┘
                   │ 检查通过后
┌──────────────────▼──────────────────────┐
│  代码生成层: jit.rs                     │
│  translate_dynamic_array_literal        │
│  translate_index (dynamic path)         │
│  translate_drop + auto-drop             │
└──────────────────┬──────────────────────┘
                   │ call "array_new_i64" 等
┌──────────────────▼──────────────────────┐
│  FFI 桥接层: runtime/array.rs           │
│  Box::into_raw() / Box::from_raw()      │
│  8 操作 × 3 类型 = 24 个 C ABI 函数    │
└──────────────────┬──────────────────────┘
                   │ 使用
┌──────────────────▼──────────────────────┐
│  底层实现: raii_demo::DynamicArray<T>   │
│  std::alloc::{alloc, realloc, dealloc}  │
│  RAII Drop: drop_in_place + dealloc     │
└─────────────────────────────────────────┘
```

#### 底层实现 — `raii_demo/src/lib.rs`

`DynamicArray<T>` 使用 `std::alloc` 的原始分配器，完全不依赖 `Vec`：

```rust
pub struct DynamicArray<T> {
    ptr: NonNull<T>,
    cap: usize,
    len: usize,
    _marker: PhantomData<T>,
}
```

**扩容策略**（`lib.rs:167-169`）：
```rust
fn grow(&mut self) {
    let new_cap = if self.cap == 0 { 1 } else { self.cap * 2 };
    // 经典翻倍策略，摊销 O(1) push
}
```

**RAII Drop**（`lib.rs:195-207`）：
```rust
impl<T> Drop for DynamicArray<T> {
    fn drop(&mut self) {
        if self.cap != 0 {
            unsafe {
                // 1. 析构所有有效元素
                ptr::drop_in_place(ptr::slice_from_raw_parts_mut(self.ptr.as_ptr(), self.len));
                // 2. 释放内存块
                let layout = Layout::array::<T>(self.cap).unwrap();
                dealloc(self.ptr.as_ptr() as *mut u8, layout);
            }
        }
    }
}
```

#### FFI 桥接层 — `src/runtime/array.rs`

Rust 与 JIT 代码之间的所有权转移使用标准 FFI 模式：

**创建**（交出所有权）：
```rust
pub extern "C" fn dynamic_array_new_i64() -> *mut DynamicArray<i64> {
    let arr = Box::new(DynamicArray::<i64>::new());
    Box::into_raw(arr)  // Rust 所有权 → 裸指针（交给 JIT 代码）
}
```

**销毁**（收回所有权）：
```rust
pub unsafe extern "C" fn dynamic_array_drop_i64(arr_ptr: *mut DynamicArray<i64>) -> i64 {
    if !arr_ptr.is_null() {
        unsafe { let _ = Box::from_raw(arr_ptr); }  // 裸指针 → Box → 触发 Drop
    }
    0
}
```

**索引**（带越界检查）：
```rust
pub unsafe extern "C" fn dynamic_array_get_ptr_i64(
    arr_ptr: *mut DynamicArray<i64>, index: usize,
) -> *mut i64 {
    let arr = unsafe { &mut *arr_ptr };
    if index >= arr.len() { ptr::null_mut() }  // 越界返回 null
    else { unsafe { arr.as_mut_ptr().add(index) } }
}
```

#### 代码生成层 — `src/jit.rs`

**创建动态数组** — `src/jit.rs:861-918`：

```rust
fn translate_dynamic_array_literal(&mut self, elems: Vec<Expr>, ty: FrontendType) -> Value {
    // 1. 根据元素类型选择函数名
    let (new_fn, push_fn, cl_elem_ty) = match elem_ty {
        FrontendType::I64 => ("array_new_i64", "array_push", types::I64),
        FrontendType::F64 => ("array_new_f64", "array_push_f64", types::F64),
        FrontendType::Complex128 =>
            ("array_new_complex128", "array_push_complex128", types::I128),
        // ...
    };

    // 2. 调用 array_new_*() → 获得 arr_ptr
    let callee = self.module.declare_function(new_fn, Linkage::Import, &sig).unwrap();
    let call = self.builder.ins().call(local_callee, &[]);
    let arr_ptr = self.builder.inst_results(call)[0];

    // 3. 逐个元素调用 array_push_*(arr_ptr, elem)
    for elem in elems {
        let val = self.translate_expr(elem);
        self.builder.ins().call(push_local_callee, &[arr_ptr, val_cast]);
    }

    arr_ptr  // 返回指向堆上 DynamicArray 的 I64 指针
}
```

**自动 drop** — `src/jit.rs:196-231`：

在函数所有语句翻译完毕后、`return` 指令之前，编译器遍历 `dynamic_arrays` 列表，对未被返回也未显式 drop 的数组按元素类型插入 drop 调用：

```rust
for (var, arr_ty) in dynamic_arrays {
    if var == *return_variable { continue; }       // 返回给调用者，不释放
    if trans.explicitly_dropped.contains(&var) { continue; }  // 已显式 drop

    let drop_func_name = match elem_ty.as_ref() {
        FrontendType::I64 | I32 | I16 | I8 => "array_drop",
        FrontendType::F64 => "array_drop_f64",
        FrontendType::Complex128 => "array_drop_complex128",
        _ => "array_drop",
    };
    // 发射 call drop_func(arr_ptr)
    trans.builder.ins().call(drop_local_callee, &[val]);
}
```

#### 编译期检查层 — `src/ownership.rs`

5 状态机（`ArrayDisposition`）：

```
Uninitialized ──(array [..])──▶ Owned ──(return var)──▶ Returned
                                   │
                                   ├──(drop())─────▶ Dropped
                                   │
                                   └──(传入函数参数)──▶ Passed
```

三种编译期错误：

| 错误 | 触发条件 | Toy 代码示例 |
|---|---|---|
| `LeakedArray` | 函数结束时变量仍为 `Owned` | `arr = array [1]; r = 0` （只创建不释放） |
| `DoubleDrop` | 对已 Dropped/Returned/Passed 的变量再次 drop | `drop(arr); drop(arr)` |
| `UseAfterDrop` | drop 不存在的变量 | `drop(x)` 其中 x 不是 DynamicArray |

#### 完整内存回收时间线

以 `DYNAMIC_ARRAY_TEST_CODE` 为例，追踪每一步的内存操作：

```toy
fn dynamic_array_test() -> (r: i64) {
    arr = array [10, 20, 30]
    array_push(arr, 40)
    r = arr[3]
}
```

| 时刻 | 操作 | 内存事件 |
|---|---|---|
| ① | `array_new_i64()` | `Box::new(DynamicArray::new())` — 在堆上分配 `DynamicArray<i64>` 结构体（ptr: dangling, cap: 0, len: 0）。`Box::into_raw()` 将裸指针交给 JIT 代码 |
| ② | `array_push(arr, 10)` | `len == cap (0==0)` → `grow()` → `alloc(Layout::array::<i64>(1))` — 在堆上分配 8 字节，cap 变为 1。`ptr::write` 写入 10，len 变为 1 |
| ③ | `array_push(arr, 20)` | `len == cap (1==1)` → `grow()` → cap 翻倍到 2 → `realloc` 为 16 字节。写入 20，len 变为 2 |
| ④ | `array_push(arr, 30)` | `len == cap (2==2)` → `grow()` → cap 翻倍到 4 → `realloc` 为 32 字节。写入 30，len 变为 3 |
| ⑤ | `array_push(arr, 40)` | `len < cap (3<4)` → 无需扩容。直接 `ptr::write` 写入 40，len 变为 4 |
| ⑥ | `r = arr[3]` | `array_get_ptr_i64(arr, 3)` — 返回 `&arr[3]` 的原始指针。JIT 端 `load I64` 读取值 40 |
| ⑦ | 函数退出 | auto-drop：`arr` 是 `DynamicArray(I64)` → 发射 `call array_drop(arr_ptr)` → `Box::from_raw(arr_ptr)` → `DynamicArray::drop()` → `drop_in_place` 析构 4 个 i64（i64 无析构器，空操作）→ `dealloc(ptr, Layout::array::<i64>(4))` 释放 32 字节堆内存 → `Box` 自身的堆内存也被释放 |

**关键观察**：
- 扩容的 `realloc` 会自动释放旧内存块，不需要手动管理
- Drop 分两步：先析构元素，再释放内存 — 对 i64 等 POD 类型第一步是空操作，但对 Complex128 等类型可以正确释放
- 整个过程中 JIT 代码只持有一个 I64 指针值，所有实际的内存操作都在 Rust 侧的 `runtime/array.rs` 中完成

#### 设计优势

| 对比维度 | 本项目的方案 | GC 方案 | 纯手动 malloc/free |
|---|---|---|---|
| 内存释放时机 | 编译期确定（函数退出自动 drop） | 运行时不确定（GC 触发） | 程序员手动决定 |
| 运行时开销 | 仅一个 `call drop_func` 指令 | 扫描/标记/整理 | 手动调用 free |
| 安全性 | 编译期静态检查（ownership checker） | 运行时安全（无 dangling pointer） | 无保证 |
| 暂停时间 | 零（无 GC） | 可能较长的 GC 暂停 | 零 |
| 易用性 | 自动 drop + 可选显式 drop() | 完全自动 | 完全手动 |

---

### 2.6 复数运算

**涉及文件**：`frontend.rs` (ComplexLiteral, Complex64/Complex128 type) → `jit.rs` (`translate_complex_literal`, `translate_complex_binop`, `is_complex`) → `type_checker.rs`

**依赖库**：`cranelift` 提供 I64/I128 类型 + `bitcast`/`stack_store`/`stack_load` 用于打包/解包

#### 存储格式

| 复数类型 | Cranelift 类型 | 内存布局 |
|---|---|---|
| Complex64 | `I64` | 低 32 位 = 实部 f32 bits，高 32 位 = 虚部 f32 bits |
| Complex128 | `I128` | 低 64 位 = 实部 f64 bits，高 64 位 = 虚部 f64 bits |

`Complex128` 使用栈槽（stack slot）实现 `I128` 值，因为 Cranelift 的 I128 不能直接通过寄存器操作。

#### 字面量创建 — `src/jit.rs:795-821`

Complex128 的创建：

```rust
FrontendType::Complex128 => {
    let re_bits = re.to_bits();
    let im_bits = im.to_bits();

    // 创建 16 字节栈槽（对齐 4 = 2^4 = 16 字节）
    let ss = self.builder.create_sized_stack_slot(StackSlotData::new(
        StackSlotKind::ExplicitSlot, 16, 4));

    // 分别存储 f64 bits 到偏移 0 和 8
    let low = self.builder.ins().iconst(types::I64, re_bits as i64);
    let high = self.builder.ins().iconst(types::I64, im_bits as i64);
    self.builder.ins().stack_store(low, ss, 0);
    self.builder.ins().stack_store(high, ss, 8);

    // 作为 I128 整体加载
    self.builder.ins().stack_load(types::I128, ss, 0)
}
```

#### 四则运算 — `src/jit.rs:998-1165`

以 Complex64 的乘法为例：

```rust
// 复数乘法：(a+bi)(c+di) = (ac-bd) + (ad+bc)i

// 1. 解包：ireduce 提取低/高 32 位 → bitcast 为 F32
let l_re = bitcast(F32, ireduce(I32, l_val));
let l_im = bitcast(F32, ireduce(I32, ushr_imm(l_val, 32)));

// 2. 逐分量 F32 运算
let ac = fmul(l_re, r_re);
let bd = fmul(l_im, r_im);
let re = fsub(ac, bd);   // ac - bd
let ad = fmul(l_re, r_im);
let bc = fmul(l_im, r_re);
let im = fadd(ad, bc);   // ad + bc

// 3. 重新打包：bitcast 回 I32 → uextend → ishl → bor
let re_bits = bitcast(I32, re);
let im_bits = bitcast(I32, im);
let re_i64 = uextend(I64, re_bits);
let im_i64 = uextend(I64, im_bits);
let im_shifted = ishl_imm(im_i64, 32);
bor(re_i64, im_shifted)   // 最终 I64 值
```

除法包含除零保护：计算分母 `c²+d²`，如果为零则替换为 1 避免除零异常。

#### 示例脚本 Walkthrough

`COMPLEX_TEST_CODE`（`src/bin/toy.rs:367-374`）：

```toy
fn complex_test() -> (status: i64) {
    c1 = 1.5 + 2.5i
    c2 = 0.5 + 0.5i
    c = c1 + c2        // 复数加法
    status = 1
}
```

1. `c1 = 1.5 + 2.5i` → `translate_complex_literal(1.5, 2.5, Complex128)` → stack_store + stack_load → I128 值
2. `c1 + c2` → `infer_type` 返回 `Complex128` → `translate_complex_binop(Add)` → 解包 → fadd 实部和虚部 → stack_store 重新打包
3. 整个运算在 CRANELIFT IR 层面完成，不调用任何外部函数

#### 设计优势

利用 Cranelift 的位操作（`bitcast`/`ireduce`/`ishl`/`bor`）和栈槽（`stack_store`/`stack_load`）原语，复数运算完全在 IR 中 inline 完成，不需要调用任何运行时函数——性能等同于手写的 C 复数运算。

---

### 2.7 外部函数调用与 MKL 集成

**涉及文件**：`runtime/registry.rs` → `jit.rs` (`translate_call`) → `type_checker.rs` (`FunctionSignature`) → `runtime/math.rs` / `runtime/mkl.rs`

**依赖库**：`cranelift-jit` 的符号解析机制 + `libc` + 可选 `intel-mkl-src`

#### 符号注册机制 — `src/runtime/registry.rs:17-80`

在 `JIT::default()` 初始化时，所有运行时函数通过 `JITBuilder.symbol()` 注册为可链接的符号：

```rust
pub fn register_builtins(builder: &mut JITBuilder) {
    builder.symbol("printf", string::printf as *const u8);
    builder.symbol("puts", string::puts as *const u8);
    builder.symbol("sin", math::toy_sin as *const u8);
    builder.symbol("array_new_i64", array::dynamic_array_new_i64 as *const u8);
    builder.symbol("array_push", array::dynamic_array_push_i64 as *const u8);
    // ... 50+ 个符号
}
```

当 JIT 代码中包含 `call "sin"` 指令时，Cranelift 的链接器在 `finalize_definitions()` 阶段根据符号名查找注册的函数指针，完成重定位——类似于动态链接器的符号解析，但发生在 JIT 编译时。

#### 函数调用翻译 — `src/jit.rs:696-759`

```rust
fn translate_call(&mut self, name: String, args: Vec<Expr>) -> Value {
    // 1. 查找函数签名
    let signature = self.type_checker.resolve_func(&name);

    // 2. 翻译参数（固定数组展开为 ptr+len）
    for arg in args {
        let val = self.translate_expr(arg);
        if should_expand { /* 展开 Array → ptr + len */ }
        arg_values.push(val);
    }

    // 3. 声明外部函数（Linkage::Import）
    let callee = self.module.declare_function(&name, Linkage::Import, &sig).unwrap();
    let local_callee = self.module.declare_func_in_func(callee, self.builder.func);

    // 4. 发射 call 指令
    let call = self.builder.ins().call(local_callee, &arg_values);
    self.builder.inst_results(call)[0]
}
```

#### MKL DGEMM 集成 — `src/runtime/mkl.rs`

**FFI 声明**：
```rust
unsafe extern "C" {
    pub fn cblas_dgemm(
        layout: i32, trans_a: i32, trans_b: i32,
        m: MklInt, n: MklInt, k: MklInt,
        alpha: f64, a: *const f64, lda: MklInt,
        b: *const f64, ldb: MklInt,
        beta: f64, c: *mut f64, ldc: MklInt,
    );
}
```

**包装函数** `toy_mkl_dgemm`：
```rust
pub unsafe extern "C" fn toy_mkl_dgemm(
    m: i64, n: i64, k: i64,
    alpha: f64, a_ptr: *const f64, a_len: usize,
    beta: f64, b_ptr: *const f64, b_len: usize,
    c_ptr: *mut f64, c_len: usize,
) -> i64 {
    // 验证数组维度
    if a_len < (m as usize) * (k as usize) { return -1; }
    if b_len < (k as usize) * (n as usize) { return -2; }
    if c_len < (m as usize) * (n as usize) { return -3; }

    unsafe {
        cblas_dgemm(
            101,  // CblasRowMajor
            111,  // CblasNoTrans
            111,  // CblasNoTrans
            m as MklInt, n as MklInt, k as MklInt,
            alpha, a_ptr, k as MklInt,   // lda = k
            b_ptr, n as MklInt,           // ldb = n
            beta, c_ptr, n as MklInt,    // ldc = n
        );
    }
    0  // 成功
}
```

Toy 代码中的固定数组自动展开为 `(ptr, len)`：
```toy
fn test_mkl(c: [f64; 4]) -> (r: i64) {
    a = [1.0, 2.0, 3.0, 4.0]  // 栈上 32 字节
    b = [5.0, 6.0, 7.0, 8.0]
    toy_mkl_dgemm(2, 2, 2, 1.0, a, 0.0, b, c)
    // ↑ a → (a_ptr, 4), b → (b_ptr, 4), c → (c_ptr, 4)
    r = 0
}
```

#### 设计优势

- **零开销 FFI**：`call "sin"` 在机器码层面就是一条 `call` 指令，与 C 调用函数无异
- **符号注册**：完全解耦——运行时函数的实现（Rust）和调用（JIT 代码）通过字符串名连接，修改运行时不需改动 JIT 编译逻辑
- **MKL 复用**：Toy 语言不需要自己实现矩阵乘法，直接享受 Intel 工程师数年优化的 BLAS 实现

---

## 第三章：完整编译器流水线

本章以 `DYNAMIC_ARRAY_TEST_CODE` 为追踪示例，展示从源码到执行的完整代码链路：

```toy
fn dynamic_array_test() -> (r: i64) {
    arr = array [10, 20, 30]
    array_push(arr, 40)
    r = arr[3]
}
```

### 3.1 入口 — `src/bin/toy.rs`

**调用链**：

```
main()                                [toy.rs:8]
  └─ Cli::parse_args()                [toy.rs:9] → cli/mod.rs:23
  └─ run_all_tests()                  [toy.rs:13]
       └─ JIT::default()              [toy.rs:57] → jit.rs:37
       └─ run_dynamic_array_test()    [toy.rs:99]
            └─ jit.compile(DYNAMIC_ARRAY_TEST_CODE)  [toy.rs:264]
            └─ mem::transmute(code_ptr) → extern "C" fn()  [toy.rs:265]
            └─ code_fn()              [toy.rs:266]
            └─ 断言 result == 40      [toy.rs:268]
```

**关键代码**（`src/bin/toy.rs:262-277`）：

```rust
fn run_dynamic_array_test(jit: &mut jit::JIT) -> Result<i64, String> {
    unsafe {
        let code_ptr = jit.compile(DYNAMIC_ARRAY_TEST_CODE)?;
        let code_fn = mem::transmute::<_, extern "C" fn() -> i64>(code_ptr);
        let result = code_fn();
        println!("Dynamic array test result: {}", result);
        if result == 40 {
            Ok(result)
        } else {
            Err(format!("Dynamic array test failed: expected 40, got {}", result))
        }
    }
}
```

`DYNAMIC_ARRAY_TEST_CODE` 是内联的 &str 常量（`src/bin/toy.rs:385-391`）。

### 3.2 解析 — `src/frontend.rs`

#### 2.2.1 AST 结构

文件 `src/frontend.rs` 包含两部分：**AST 枚举定义**（`Expr` 和 `Type`）和 **PEG 语法**（`peg::parser!` 宏）。

**Expr 枚举** (`frontend.rs:2-28`) — 27 个变体，表示所有可能的表达式节点：

```rust
pub enum Expr {
    Literal(String, Type),              // 字面量 "42", "3.14" + 类型
    StringLiteral(String),              // "hello" 字符串
    ComplexLiteral(f64, f64, Type),     // 1.5 + 2.5i (实部, 虚部, 类型)
    ArrayLiteral(Vec<Expr>, Type),      // [1, 2, 3] 固定数组
    DynamicArrayLiteral(Vec<Expr>, Type), // array [1, 2, 3] 动态数组
    Identifier(String),                 // 变量名
    Assign(String, Box<Expr>),          // x = expr
    Eq/Ne/Lt/Le/Gt/Ge(...),            // 比较运算
    Add/Sub/Mul/Div(...),              // 算术运算
    IfElse(Box<Expr>, Vec<Expr>, Vec<Expr>), // if-else
    WhileLoop(Box<Expr>, Vec<Expr>),   // while 循环
    Call(String, Vec<Expr>),           // 函数调用
    Index(Box<Expr>, Box<Expr>),       // arr[idx] 索引
    GlobalDataAddr(String),            // &name 全局数据地址
    Cast(Box<Expr>, Type),             // expr as Type
    Drop(String),                      // drop(var) 显式释放
}
```

**Type 枚举** (`frontend.rs:30-46`) — 12 个变体：

```rust
pub enum Type {
    I8, I16, I32, I64, I128,
    F32, F64,
    String,
    Complex64, Complex128,
    Array(Box<Type>, usize),           // 固定数组 [T; N]
    DynamicArray(Box<Type>),           // 动态数组 array<T>
}
```

#### 2.2.2 解析过程

PEG 语法解析从 `parser::function(input)` 开始（`jit.rs:68-69`），返回 `(name, params, return_info, stmts)`。

**顶层规则 `function()`** (`frontend.rs:62-87`)：

```
fn name(params...) -> (ret: Type) { \n   // <-- 匹配函数签名
    stmts...                              // <-- 匹配函数体
}
```

解析过程：
1. `"fn" _ name:identifier()` — 匹配 `fn` 关键词 + 函数名
2. `"(" params:(...) ")"` — 匹配参数列表 `(name: Type, ...)`
3. `"->" _ "(" ret:(...) ")" ` — 匹配返回值 `-> (name: Type)`
4. `"{" _ "\n" stmts:statements() _ "}"` — 匹配函数体

**`statements()` 规则** (`frontend.rs:92-93`) — 0 个或多个 `statement()`：

```
rule statements() -> Vec<Expr>
    = s:(statement()*) { ... }
```

**`expression()` 规则** (`frontend.rs:108-113`) — 按优先级尝试匹配：

```
rule expression() -> Expr
    = if_else()        // 最高优先级
    / while_loop()
    / "drop" (...)     // drop(var)
    / assignment()     // a = expr
    / binary_op()      // 二元运算（含函数调用、字面量等）
```

**`binary_op()` 的 `precedence!{}`** (`frontend.rs:138-159`) — 操作符优先级从高到低：

```
Level 1: == != < <= > >=      (比较)
Level 2: + -                  (加减)
Level 3: * /                  (乘除)
Level 4: as Type              (类型转换)
Level 5: arr[idx]             (索引)
Level 6: func(args) / ident  (函数调用 / 标识符)
Level 7: literal / &name     (字面量 / 全局地址)
Level 8: ( expr )            (括号)
```

**`literal()` 规则** (`frontend.rs:195-202`) — 字面量的匹配顺序：

```
rule literal() -> Expr
    = string_literal()         // "hello"
    / complex_literal()        // 1.5 + 2.5i
    / dynamic_array_literal()  // array [1, 2, 3]
    / array_literal()          // [1, 2, 3]
    / float_literal            // 3.14 → F64
    / integer_literal          // 42 → I64
    / "&" identifier()         // &global_data
```

#### 2.2.3 示例代码的 AST 输出

对本文追踪的示例代码：

```toy
fn dynamic_array_test() -> (r: i64) {
    arr = array [10, 20, 30]
    array_push(arr, 40)
    r = arr[3]
}
```

解析后返回的四元组 `(name, params, return_info, stmts)`：

```
name = "dynamic_array_test"
params = []                                           // 无参数
return_info = ("r", Type::I64)
stmts = [
    Expr::Assign(
        "arr",
        Expr::DynamicArrayLiteral(                    // array [10, 20, 30]
            [Literal("10", I64), Literal("20", I64), Literal("30", I64)],
            Type::I64
        )
    ),
    Expr::Call(                                        // array_push(arr, 40)
        "array_push",
        [Identifier("arr"), Literal("40", I64)]
    ),
    Expr::Assign(                                      // r = arr[3]
        "r",
        Expr::Index(Identifier("arr"), Literal("3", I64))
    )
]
```

### 3.3 常量折叠 — `src/optimizer.rs`

在 `jit.rs:72` 调用：

```rust
let stmts = optimizer::fold_constants_in_stmts(stmts);
```

**核心函数** `fold_constants()` (`optimizer.rs:13-63`)：

对每种 `Expr` 变体进行递归匹配，尝试在编译期计算常量表达式：

- **算术运算**：`1+2` → `Literal("3", I64)`，`x+0` → `x`，`0*x` → `Literal("0", I64)`
- **比较运算**：`5==5` → `Literal("1", I64)`，`x==x` → `Literal("1", I64)`
- 对于无法折叠的比较运算，`fold_cmp` 保持原始比较运算符不变（如 `x < y` 仍为 `Lt`，不会被错误转为 `Eq`）。
- **类型转换**：`42 as f64` → `Literal("42", F64)`

**代数恒等式**实现（`optimizer.rs:93-199`）：

| 模式 | 折叠结果 | 函数 |
|---|---|---|
| `0 + x` 或 `x + 0` | `x` | `fold_add` |
| `0 * x` 或 `x * 0` | `0` | `fold_mul` |
| `1 * x` 或 `x * 1` | `x` | `fold_mul` |
| `x - 0` | `x` | `fold_sub` |
| `x / 1` | `x` | `fold_div` |
| `0 / x` (x != 0) | `0` | `fold_div` |

**对本示例的影响**：所有字面量都是 10/20/30/40/3 — 它们单独出现没有参与常量间的运算，所以 optimizer 透传，AST 不变。

**局限性**：只处理 `I64`/`F64` 字面量，不追踪变量的实际值，不跨语句优化。

### 3.4 所有权检查 — `src/ownership.rs`

在 `jit.rs:75-84` 调用：

```rust
{
    let mut checker = ownership::OwnershipChecker::new();
    let errors = checker.analyze_function(&params, &stmts, &the_return.0);
    if !errors.is_empty() {
        let error_msgs: Vec<String> = errors.iter()
            .map(|e| e.to_string())
            .collect();
        return Err(format!("ownership errors:\n{}", error_msgs.join("\n")));
    }
}
```

#### 2.4.1 状态机

`ArrayDisposition` 枚举 (`ownership.rs:9-20`) 定义了 5 种所有权状态：

```
Uninitialized ──(array [..])──▶ Owned ──(return)──▶ Returned
                                   │
                                   ├──(drop())─────▶ Dropped
                                   │
                                   └──(传递为非借用函数参数)──▶ Passed
```

内置函数（`array_push`/`array_pop`/`array_len`/`array_cap`/`array_get_ptr`/`array_set`
及其 `_f64`/`_complex128` 变体，以及所有数学函数、IO 函数）属于**借用**，
不会将 `Owned` 变为 `Passed`。只有用户自定义函数（非内置）才会转移所有权。

#### 2.4.2 分析流程

`OwnershipChecker::analyze_function()` (`ownership.rs:79-99`)：

1. 遍历每条语句，调用 `analyze_expr()` 更新状态
2. 遍历结束后调用 `check_for_leaks()` — 任何仍处于 `Owned` 状态的数组报 `LeakedArray`

**对本示例的逐语句分析**：

| 语句 | 状态变化 |
|---|---|
| `arr = array [10, 20, 30]` | `get_rhs_array_info()` 检测到 `DynamicArrayLiteral` → 插入 `arr: Owned` |
| `array_push(arr, 40)` | `array_push` 是内置借用函数 → `arr` 保持 `Owned`（不转移所有权） |
| `r = arr[3]` | `Index` 表达式 — 不改变所有权状态；同时检查 `arr` 未被 drop |
| `drop(arr)` | `arr` 从 `Owned` 变为 `Dropped`，避免泄漏 |
| 函数结束 | `check_for_leaks()` — 没有 `Owned` 的数组 → 无错误 |

**如果写成泄漏代码会怎样**：

```toy
fn leak_test() -> (r: i64) {
    arr = array [1, 2, 3]       // arr: Owned
    r = 0                        // arr 既没被 drop，也没返回
}
// → OwnershipError::LeakedArray { name: "arr" }
```

#### 2.4.3 三种错误类型

| 错误 | 触发条件 |
|---|---|
| `LeakedArray { name }` | 函数结束时 `name` 仍为 `Owned` |
| `UseAfterDrop { name }` | 在 drop 之后使用数组（包括索引访问 `arr[i]` 或作为函数参数传递已 Passed 的数组） |
| `DoubleDrop { name }` | drop 一个已 Returned/Dropped/Passed 的变量 |

### 3.5 类型检查器 — `src/type_checker.rs`

#### 2.5.1 TypeChecker 结构

`TypeChecker` (`type_checker.rs:11-13`) 维护一个 `HashMap<String, FunctionSignature>`，在构造时通过 `register_builtins()` 预填充所有已知函数的签名。

`FunctionSignature` (`type_checker.rs:4-9`)：

```rust
pub struct FunctionSignature {
    pub params: Vec<Type>,
    pub ret: Type,
    pub is_external: bool,  // true: 外部函数，数组参数展开为 (ptr, len)
}
```

`is_external` 是关键标志：当 Toy 函数调用外部 C 函数时，固定数组 `[T; N]` 会被展开为两个参数：指针 + 长度。这在 `jit.rs:709-735` 的 `translate_call` 中处理。

#### 2.5.2 类型推导 — `infer_type()`

`infer_type(expr, get_var_type)` (`type_checker.rs:288-368`) 接收一个表达式和一个闭包（用于查询变量的类型），返回该表达式的类型。

核心逻辑：

- `Literal(_, ty)` → 直接返回 `ty`
- `DynamicArrayLiteral(elems, _)` → `DynamicArray(elem_ty)`，从第一个元素推导
- `Call(name, _)` → 硬编码的函数返回类型匹配（`sin` → F64, `array_push` → I64, `array_new_i64` → `DynamicArray(I64)` 等）
- `Index(base, _)` → `Array(inner, _)` → `inner`；`DynamicArray(inner)` → `inner`
- `Add(lhs, _)` 等 → `infer_type(lhs)` — 算术运算结果类型 = 左操作数类型

此函数被两个地方调用：
1. `declare_variables()` (`jit.rs:1225-1226`) — 声明变量时推断类型
2. `translate_expr` 的各分支 (`jit.rs:327-329` 等) — 判断是否需要进行复数运算

### 3.6 JIT 编译器 — `src/jit.rs`

这是整个项目最核心的文件（约 1250 行），包含：
- `JIT` 结构体 — JIT 编译器的生命周期
- `FunctionTranslator` — AST → Cranelift IR 的翻译器
- `declare_variables()` — 变量前向扫描

#### 2.6.1 JIT 初始化 — `JIT::default()`

`src/jit.rs:37-62`：

```rust
impl Default for JIT {
    fn default() -> Self {
        // Step 1: 构建 Cranelift 设置
        let mut flag_builder = settings::builder();
        flag_builder.set("use_colocated_libcalls", "false").unwrap();
        flag_builder.set("is_pic", "false").unwrap();

        // Step 2: 自动检测宿主机器 ISA (x86-64/ARM64/...)
        let isa_builder = cranelift_native::builder().unwrap_or_else(|msg| {
            panic!("host machine is not supported: {}", msg);
        });
        let isa = isa_builder
            .finish(settings::Flags::new(flag_builder))
            .unwrap();

        // Step 3: 创建 JITBuilder，注册所有运行时函数
        let mut builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());
        runtime::register_builtins(&mut builder);  // ← 注册所有 extern "C" 函数

        // Step 4: 创建 JITModule
        let module = JITModule::new(builder);

        Self {
            builder_context: FunctionBuilderContext::new(),
            ctx: module.make_context(),
            data_description: DataDescription::new(),
            module,
            type_checker: TypeChecker::new(),  // ← 初始化类型检查器
        }
    }
}
```

**关键点**：`cranelift_native::builder()` 自动检测宿主 CPU 架构，生成针对当前机器的优化代码。`runtime::register_builtins()` 将所有 C ABI 函数指针注册为可链接的符号。

#### 2.6.2 `compile()` — 编译流水线编排

`src/jit.rs:66-111`：

```rust
pub fn compile(&mut self, input: &str) -> Result<*const u8, String> {
    // 1. 解析 → AST
    let (name, params, the_return, stmts) =
        parser::function(input).map_err(|e| e.to_string())?;

    // 2. 常量折叠
    let stmts = optimizer::fold_constants_in_stmts(stmts);

    // 3. 所有权检查
    {
        let mut checker = ownership::OwnershipChecker::new();
        let errors = checker.analyze_function(&params, &stmts, &the_return.0);
        if !errors.is_empty() { return Err(...); }
    }

    // 4. AST → Cranelift IR
    self.translate(name.clone(), params, the_return, stmts)?;

    // 5. 声明函数（让其他函数可以调用）
    let id = self.module.declare_function(&name, Linkage::Export, &self.ctx.func.signature)
        .map_err(|e| e.to_string())?;

    // 6. 定义函数（IR → 机器码）
    self.module.define_function(id, &mut self.ctx)
        .map_err(|e| e.to_string())?;

    // 7. 清理上下文
    self.module.clear_context(&mut self.ctx);

    // 8. 最终化（解析所有符号引用）
    self.module.finalize_definitions().unwrap();

    // 9. 返回机器码指针
    let code = self.module.get_finalized_function(id);
    Ok(code)
}
```

#### 2.6.3 `to_cranelift_type()` — 类型映射

`src/jit.rs:241-256`：

```rust
fn to_cranelift_type(t: &FrontendType) -> types::Type {
    match t {
        FrontendType::I8 => types::I8,
        FrontendType::I16 => types::I16,
        FrontendType::I32 => types::I32,
        FrontendType::I64 => types::I64,
        FrontendType::I128 => types::I128,
        FrontendType::F32 => types::F32,
        FrontendType::F64 => types::F64,
        FrontendType::String => types::I64,          // 指针
        FrontendType::Complex64 => types::I64,       // 打包的 2×f32
        FrontendType::Complex128 => types::I128,     // 打包的 2×f64
        FrontendType::Array(_, _) => types::I64,     // 指针
        FrontendType::DynamicArray(_) => types::I64, // 指向堆上 DynamicArray 的指针
    }
}
```

**注意**：`String`、`Array`、`DynamicArray` 都映射为 `I64`（即 64 位指针）。`Complex64` 打包为 `I64`（低 32 位实部 + 高 32 位虚部）。`Complex128` 打包为 `I128`。

#### 2.6.4 `translate()` — 函数翻译入口

`src/jit.rs:136-238`：

```rust
fn translate(&mut self, name, params, the_return, stmts) -> Result<(), String> {
    // 1. 构建函数签名
    for (_, ty) in &params {
        self.ctx.func.signature.params.push(AbiParam::new(to_cranelift_type(ty)));
    }
    self.ctx.func.signature.returns.push(AbiParam::new(to_cranelift_type(&the_return.1)));

    // 2. 创建基本块
    let mut builder = FunctionBuilder::new(&mut self.ctx.func, &mut self.builder_context);
    let entry_block = builder.create_block();
    builder.append_block_params_for_function_params(entry_block);
    builder.switch_to_block(entry_block);
    builder.seal_block(entry_block);

    // 3. 声明所有变量（前向扫描）
    let variables = declare_variables(&mut builder, &params, &stmts, entry_block, &the_return);

    // 4. 创建 FunctionTranslator
    let mut trans = FunctionTranslator {
        builder, variables, module: &mut self.module,
        current_func_name: name, current_func_ret_type: ...,
        string_counter: 0, type_checker: &self.type_checker,
        dynamic_arrays: Vec::new(), explicitly_dropped: Vec::new(),
    };

    // 5. 逐条翻译
    for expr in stmts {
        trans.translate_expr(expr);
    }

    // 6. 自动 drop + return
    // ... (见 2.6.6)
}
```

#### 2.6.5 `FunctionTranslator` 结构体

`src/jit.rs:269-279`：

```rust
struct FunctionTranslator<'a> {
    builder: FunctionBuilder<'a>,                         // Cranelift IR 构建器
    variables: HashMap<String, (Variable, FrontendType)>, // 变量名 → (Cranelift变量, 类型)
    module: &'a mut JITModule,                            // JIT 模块（声明函数/数据用）
    current_func_name: String,                            // 当前函数名（递归用）
    current_func_ret_type: types::Type,                   // 当前函数返回类型
    string_counter: usize,                                // 字符串命名计数器
    type_checker: &'a TypeChecker,                        // 类型检查器引用
    dynamic_arrays: Vec<(Variable, FrontendType)>,        // 追踪所有 DynamicArray 变量
    explicitly_dropped: Vec<Variable>,                    // 已显式 drop 的变量
}
```

**`dynamic_arrays` 和 `explicitly_dropped` 的作用**：记录函数中所有 DynamicArray 变量及其类型，以便在函数退出时自动插入 `drop` 调用（跳过已显式 drop 的变量和返回变量）。

#### 2.6.6 翻译追踪示例的三个语句

对本示例的三条语句逐一追踪 `translate_expr()` 的执行：

##### 语句 1：`arr = array [10, 20, 30]`

进入 `translate_assign()` (`jit.rs:545-571`) → 翻译 RHS → 进入 `translate_dynamic_array_literal()` (`jit.rs:861-918`)：

```rust
fn translate_dynamic_array_literal(&mut self, elems: Vec<Expr>, ty: FrontendType) -> Value {
    // 1. 根据元素类型选择函数名
    let (new_fn, push_fn, cl_elem_ty) = match elem_ty {
        FrontendType::I64 => ("array_new_i64", "array_push", types::I64),
        FrontendType::F64 => ("array_new_f64", "array_push_f64", types::F64),
        FrontendType::Complex128 => ("array_new_complex128", "array_push_complex128", types::I128),
        // ...
    };

    // 2. 调用 array_new_i64() 创建空数组
    let mut sig = self.module.make_signature();
    sig.returns.push(AbiParam::new(types::I64));
    let callee = self.module.declare_function(new_fn, Linkage::Import, &sig).unwrap();
    let local_callee = self.module.declare_func_in_func(callee, self.builder.func);
    let call = self.builder.ins().call(local_callee, &[]);
    let arr_ptr = self.builder.inst_results(call)[0];  // arr_ptr: I64

    // 3. 对每个元素调用 array_push(arr_ptr, elem)
    for elem in elems {
        let val = self.translate_expr(elem);
        let val_cast = self.translate_cast(val, cl_elem_ty);
        self.builder.ins().call(push_local_callee, &[arr_ptr, val_cast]);
    }

    arr_ptr  // 返回指向 DynamicArray 的指针
}
```

**关键**：这里生成的 Cranelift IR 是一系列 `call` 指令，调用的是运行时层 `runtime/array.rs` 中实现的 C ABI 函数。

然后回到 `translate_assign()`，`def_var(arr, arr_ptr)` 将 SSA 值赋给变量，并因为类型是 `DynamicArray(I64)`，将 `arr` 加入 `self.dynamic_arrays` 追踪列表。

##### 语句 2：`array_push(arr, 40)`

进入 `translate_call()` (`jit.rs:696-759`)：

```rust
fn translate_call(&mut self, name: String, args: Vec<Expr>) -> Value {
    let mut sig = self.module.make_signature();

    // 1. 查找函数签名
    let signature = self.type_checker.resolve_func(&name);

    // 2. 翻译每个参数
    let mut arg_values = Vec::new();
    for arg in args {
        let arg_ty = type_checker::infer_type(&arg, &|n| self.variables.get(n).map(|(_, t)| t.clone()));
        let val = self.translate_expr(arg);

        // 如果是固定数组且目标是外部函数 → 展开为 (ptr, len)
        let should_expand = if let FrontendType::Array(_, _) = arg_ty {
            signature.map(|s| s.is_external).unwrap_or(false)
        } else { false };

        if should_expand {
            // ... push ptr, then push len
        } else {
            arg_values.push(val);
            sig.params.push(AbiParam::new(self.builder.func.dfg.value_type(val)));
        }
    }

    // 3. 确定返回类型
    let ret_ty = if let Some(s) = signature { to_cranelift_type(&s.ret) }
                 else { types::I64 };
    sig.returns.push(AbiParam::new(ret_ty));

    // 4. 声明并调用函数
    let callee = self.module.declare_function(&name, Linkage::Import, &sig).unwrap();
    let local_callee = self.module.declare_func_in_func(callee, self.builder.func);
    let call = self.builder.ins().call(local_callee, &arg_values);
    self.builder.inst_results(call)[0]
}
```

**参数展开逻辑**：`Array` 类型的参数，在调用 `is_external=true` 的函数时，会自动展开为 `(ptr, len)` 两个参数。这使得 Toy 语言的固定数组 `[f64; 4]` 可以传递给 C 函数。但 `DynamicArray` 不走展开路径（作为单个指针传递）。

##### 语句 3：`r = arr[3]`

进入 `translate_assign()` → RHS 进入 `translate_index()` (`jit.rs:920-996`)：

```rust
fn translate_index(&mut self, base: Expr, idx: Expr) -> Value {
    let base_ty = type_checker::infer_type(&base, &|n| ...);
    let (elem_ty, len, is_dynamic) = match base_ty {
        FrontendType::Array(t, l) => (*t, l, false),
        FrontendType::DynamicArray(t) => (*t, 0, true),  // ← 动态数组
        // ...
    };

    let base_val = self.translate_expr(base);  // arr_ptr
    let idx_val = self.translate_expr(idx);     // 3_i64

    let cl_elem_ty = to_cranelift_type(&elem_ty);  // I64

    if is_dynamic {
        // 1. 根据元素类型选择 get_ptr 函数
        let get_ptr_fn = match elem_ty {
            FrontendType::I64 => "array_get_ptr",
            FrontendType::F64 => "array_get_ptr_f64",
            FrontendType::Complex128 => "array_get_ptr_complex128",
            // ...
        };

        // 2. 调用 array_get_ptr(arr_ptr, index) → element pointer
        let callee = self.module.declare_function(get_ptr_fn, Linkage::Import, &sig).unwrap();
        let local_callee = self.module.declare_func_in_func(callee, self.builder.func);
        let call = self.builder.ins().call(local_callee, &[base_val, idx_val_i64]);
        let addr = self.builder.inst_results(call)[0];

        // 3. 越界检查：若指针为空则触发 trap
        self.builder.ins().trapz(addr, TrapCode::unwrap_user(1));

        // 4. 从元素指针加载值
        self.builder.ins().load(cl_elem_ty, MemFlags::new(), addr, 0)
    }
    // ... 固定数组路径含 inline 边界检查
}
```

**关键安全机制**：`array_get_ptr` 在索引越界时返回 `null`，JIT 代码通过 `trapz` 指令检查：若为 null 则触发 CPU trap，程序终止（而非继续执行导致未定义行为）。

#### 2.6.7 自动 drop

在 `translate()` 末尾 (`jit.rs:196-231`)，遍历所有被追踪的 DynamicArray：

```rust
let dynamic_arrays = trans.dynamic_arrays.clone();
for (var, arr_ty) in dynamic_arrays {
    // 跳过返回变量（已转移所有权给调用者）
    if var == *return_variable { continue; }
    // 跳过已显式 drop 的数组
    if trans.explicitly_dropped.contains(&var) { continue; }

    if let FrontendType::DynamicArray(elem_ty) = arr_ty {
        let drop_func_name = match elem_ty.as_ref() {
            FrontendType::I64 | FrontendType::I32 | FrontendType::I16 | FrontendType::I8 => "array_drop",
            FrontendType::F64 => "array_drop_f64",
            FrontendType::Complex128 => "array_drop_complex128",
            _ => "array_drop",
        };
        // 发射 call drop_func(var)
        // ...
    }
}
```

**对本示例**：`arr` 的所有权状态已被所有权检查器确认为 `Passed`（传入 `array_push`），且 `array_push` 不接收所有权，因此 `arr` 在 JIT 翻译中仍在 `dynamic_arrays` 列表中。但由于 `arr` 不等于返回变量且不在 `explicitly_dropped` 中，它**会被自动 drop**。

> **注意**：所有权检查器将 `array_push(arr, 40)` 中 `arr` 标记为 `Passed`，这在语义上意味着"所有权转移给被调用函数"。但实际上 `array_push` 并不获取所有权——这个语义上的不一致源于所有权检查器的保守策略。实际内存安全由运行时 `array_drop` 保证。

### 3.7 运行时层 — `src/runtime/`

#### 2.7.1 注册中心 — `registry.rs`

`register_builtins()` (`registry.rs:17-80`) 是所有 C ABI 函数的注册入口，通过 `JITBuilder.symbol(name, fn_ptr)` 将 Rust 函数指针按字符串名注册：

```rust
pub fn register_builtins(builder: &mut JITBuilder) {
    // IO
    builder.symbol("printf", string::printf as *const u8);
    builder.symbol("puts", string::puts as *const u8);
    // ...
    // DynamicArray (i64)
    builder.symbol("array_new_i64", array::dynamic_array_new_i64 as *const u8);
    builder.symbol("array_push", array::dynamic_array_push_i64 as *const u8);
    // ...
    // DynamicArray (f64)
    builder.symbol("array_new_f64", array::dynamic_array_new_f64 as *const u8);
    builder.symbol("array_push_f64", array::dynamic_array_push_f64 as *const u8);
    // ...
    // Math
    builder.symbol("sin", math::toy_sin as *const u8);
    // ...
    // MKL (条件编译)
    #[cfg(feature = "mkl")]
    register_mkl(builder);
}
```

当 JIT 代码中包含 `call sin` 指令时，Cranelift 的链接器根据符号名 `"sin"` 查找此处注册的函数指针，完成重定位。

#### 2.7.2 动态数组运行时 — `array.rs`

文件 `src/runtime/array.rs` 提供了三组类型特化的函数族，每组 8 个函数：

| 函数 | i64 版本 | f64 版本 | complex128 版本 |
|---|---|---|---|
| 创建 | `dynamic_array_new_i64()` | `dynamic_array_new_f64()` | `dynamic_array_new_complex128()` |
| 压入 | `dynamic_array_push_i64()` | `dynamic_array_push_f64()` | `dynamic_array_push_complex128()` |
| 弹出 | `dynamic_array_pop_i64()` | `dynamic_array_pop_f64()` | `dynamic_array_pop_complex128()` |
| 长度 | `dynamic_array_len_i64()` | `dynamic_array_len_f64()` | `dynamic_array_len_complex128()` |
| 容量 | `dynamic_array_cap_i64()` | `dynamic_array_cap_f64()` | `dynamic_array_cap_complex128()` |
| 取址 | `dynamic_array_get_ptr_i64()` | `dynamic_array_get_ptr_f64()` | `dynamic_array_get_ptr_complex128()` |
| 设值 | `array_set()` | `array_set_f64()` | `array_set_complex128()` |
| 析构 | `dynamic_array_drop_i64()` | `dynamic_array_drop_f64()` | `dynamic_array_drop_complex128()` |

以 i64 版本的 `new` 和 `drop` 为例：

`src/runtime/array.rs:5-9` — 创建：
```rust
pub extern "C" fn dynamic_array_new_i64() -> *mut DynamicArray<i64> {
    let arr = Box::new(DynamicArray::<i64>::new());
    Box::into_raw(arr)  // 返回裸指针，所有权交给调用者
}
```

`src/runtime/array.rs:74-80` — 析构：
```rust
pub unsafe extern "C" fn dynamic_array_drop_i64(arr_ptr: *mut DynamicArray<i64>) -> i64 {
    if !arr_ptr.is_null() {
        unsafe { let _ = Box::from_raw(arr_ptr); }  // 重建 Box，触发 Drop
    }
    0
}
```

`Box::into_raw` + `Box::from_raw` 是 Rust FFI 的标准所有权转移模式：`new` 将所有权从 Rust 移交给 JIT 代码（以裸指针形式），`drop` 将所有权收回 Rust 并释放。

#### 2.7.3 数学函数 — `math.rs`

`src/runtime/math.rs`：所有函数都是 `f64 -> f64` 的简单包装，使用 `libc::c_double`：

```rust
pub extern "C" fn toy_sin(x: c_double) -> c_double { x.sin() }
pub extern "C" fn toy_cos(x: c_double) -> c_double { x.cos() }
// sin, cos, tan, sqrt, pow, exp, log, ceil, floor 共 9 个
```

#### 2.7.4 MKL 矩阵乘法 — `mkl.rs`

`src/runtime/mkl.rs:26-73` — `toy_mkl_dgemm` 接收数组指针 + 长度，进行维度校验后调用 Intel MKL 的 `cblas_dgemm`。

Toy 语言中固定数组参数 `[f64; N]` 会被展开为 `(ptr, len)`，与 `toy_mkl_dgemm` 的签名匹配：

```toy
fn test_mkl(c: [f64; 4]) -> (r: i64) {
    a = [1.0, 2.0, 3.0, 4.0]
    b = [5.0, 6.0, 7.0, 8.0]
    toy_mkl_dgemm(2, 2, 2, 1.0, a, 0.0, b, c)  // a, b, c 展开为 (ptr, len)
    r = 0
}
```

#### 2.7.5 RAII 动态数组实现 — `raii_demo/src/lib.rs`

`raii_demo/src/lib.rs` 提供了 `DynamicArray<T>` 的完整实现（约 330 行），关键特性：

- **手动内存管理**：使用 `std::alloc::{alloc, realloc, dealloc}` 而非 `Vec`，完全控制分配
- **RAII Drop**：`Drop` 实现中先析构所有元素（`ptr::drop_in_place`），再释放内存块
- **扩容策略**：`grow()` 中容量为 0 时初始化为 1，否则翻倍
- **Deref/DerefMut**：实现 `Deref<Target = [T]>`，使 `DynamicArray<T>` 可直接当切片使用
- **迭代器**：`IntoIter` 通过 `mem::forget(self)` 接管所有权，并在迭代器的 `Drop` 中释放
- **线程安全**：`unsafe impl Send/Sync`（当 T 满足条件时）

#### JIT 编译器 Bug 修复（审查中修正）

在代码审查中修复了以下问题：

| 严重度 | 问题 | 位置 | 修复 |
|--------|------|------|------|
| **严重** | `fold_cmp` 将不能折叠的比较全部转为 `Eq`（`x < y` → `x == y`） | `optimizer.rs:228,233` | 添加 `default` 构造闭包参数，保持原始比较运算符 |
| **严重** | 整数除法使用 `udiv`（无符号），`-10/3` 结果错误 | `jit.rs:389` | 改为 `sdiv`（有符号除法） |
| **中等** | 所有权检查将所有调用标记为 `Passed`（`array_push` 等借用函数不应转移所有权） | `ownership.rs:136-147` | 添加内置借用函数白名单 |
| **中等** | `array_drop` 对 I32/I16/I8 类型不匹配（调用 `DynamicArray<i64>` 的 drop） | `jit.rs:211,589` | 仅匹配 I64，添加注释说明限制 |
| **中等** | `array_set` 等函数缺少按元素类型分发 | `jit.rs:697` | 添加 `dispatch_array_fn()` 辅助方法 |
| **低** | i128 字面量栈槽对齐 256 字节→修正为 16 字节 | `jit.rs:308` | `8` → `4` |
| **低** | 所有权检查未检测 Index 上的 use-after-drop | `ownership.rs:169-174` | 检查 `Dropped`/`Passed` 状态 |

#### 性能基准测试结果

| 基准测试 | 时间 | 对比 |
|----------|------|------|
| `jit_sin` (JIT 编译的 sin(2.0)) | 9.61 ns | native_sin: 9.11 ns (+5.5%) |
| `jit_sum_array` (JIT 固定数组求和) | 6.10 ns | — |
| `jit_dynamic_array` (JIT 动态数组 push×100) | 1.14 µs | native DynamicArray: 712 ns (+60%) |
| `raii_array_push` (DynamicArray push×1000) | 1.84 µs | std::Vec push×1000: 1.56 µs (+18%) |
| `raii_array_iter` (DynamicArray 迭代求和) | 70.3 ns | std::Vec 迭代: 70.4 ns (持平) |

**分析**：Cranelift JIT 生成的机器码在数学运算上接近原生性能（仅 5% 开销）。
主要瓶颈在 FFI 边界——JIT 代码每次调用 runtime 函数（如 `array_push`）需通过 `extern "C"`，
无法内联优化，导致动态数组操作有约 60% 开销。DynamicArray 本身与 `std::Vec` 性能差距在 18% 以内。

### 3.8 执行 — `mem::transmute`

`src/bin/toy.rs:265`：

```rust
let code_fn = mem::transmute::<_, extern "C" fn() -> i64>(code_ptr);
let result = code_fn();
```

`code_ptr` 是 `*const u8`（指向机器码的原始指针），`mem::transmute` 将其强制转换为函数指针。这之所以能工作，是因为：

1. Cranelift JIT 生成的机器码遵循**宿主平台的 C 调用约定**（x86-64 System V / Windows x64 calling convention）
2. `extern "C" fn() -> i64` 的函数指针在底层就是代码地址
3. 调用时，CPU 直接执行该地址处的机器指令

---

## 第四章：端到端完整追踪

从 `cargo run -- --test` 到 `DYNAMIC_ARRAY_TEST_CODE` 执行完毕，完整时间线：

| 阶段 | 输入 | 处理 | 输出 | 关键文件:行号 |
|---|---|---|---|---|
| **CLI 解析** | `--test` 参数 | `Cli::parse_args()` 创建 Cli 结构体 | `cli.test = true` | `toy.rs:9`, `cli/mod.rs:22` |
| **调度测试** | `cli.test == true` | `run_all_tests()` 被调用 | 进入动态数组测试 | `toy.rs:13,99` |
| **创建 JIT** | — | `JIT::default()` → ISA 检测 + 注册运行时函数 + 初始化 TypeChecker | `JIT` 实例 | `jit.rs:37-62`, `registry.rs:17-80` |
| **解析** | `DYNAMIC_ARRAY_TEST_CODE` 字符串 | `parser::function()` → PEG 语法匹配 | `(name="dynamic_array_test", params=[], ret=("r",I64), stmts=[Assign, Call, Assign])` | `jit.rs:68-69`, `frontend.rs:62-241` |
| **常量折叠** | AST (3 条语句) | `fold_constants_in_stmts()` → 递归遍历 | 透传（无折叠机会） | `jit.rs:72`, `optimizer.rs:8-63` |
| **所有权检查** | AST | `OwnershipChecker::analyze_function()` → 状态机跟踪 | `[]` (无错误)：arr 被 array_push 标记为 Passed | `jit.rs:75-84`, `ownership.rs:79-99` |
| **声明变量** | AST + 函数签名 | `declare_variables()` → 前向扫描 Assign 节点 | `variables = {"r": (var_r, I64), "arr": (var_arr, DynamicArray(I64))}` | `jit.rs:169`, `jit.rs:1169-1247` |
| **翻译语句 1** | `arr = array [10, 20, 30]` | `translate_assign()` → `translate_dynamic_array_literal()` → `call array_new_i64` + 3× `call array_push` | `arr_ptr: I64` 值 + `dynamic_arrays += [(arr, DynamicArray(I64))]` | `jit.rs:545-571,861-918` |
| **翻译语句 2** | `array_push(arr, 40)` | `translate_call()` → `resolve_func("array_push")` → `call array_push(arr_ptr, 40)` | 返回值 `0: I64` | `jit.rs:696-759` |
| **翻译语句 3** | `r = arr[3]` | `translate_assign()` → `translate_index()` → `call array_get_ptr(arr_ptr, 3)` → `trapz` → `load I64` | `40: I64` 赋值给 `r` | `jit.rs:545-571,920-996` |
| **自动 drop** | `dynamic_arrays = [(arr, DynamicArray(I64))]` | 遍历：arr ≠ 返回值 & arr ∉ explicitly_dropped → `call array_drop(arr)` | 插入 drop 指令 | `jit.rs:196-231` |
| **return** | `r` 变量 | `use_var(r)` → `ins().return_(&[r_value])` | return 指令 | `jit.rs:190-233` |
| **定义函数** | Cranelift IR (已构建) | `module.declare_function()` → `module.define_function()` → `module.clear_context()` → `module.finalize_definitions()` | 机器码就绪 | `jit.rs:90-108` |
| **获取代码指针** | 函数 ID | `module.get_finalized_function(id)` | `*const u8` 机器码指针 | `jit.rs:108` |
| **执行** | `*const u8` | `mem::transmute::<_, extern "C" fn() -> i64>(code_ptr)` → `code_fn()` | CPU 执行机器码，返回 `40` | `toy.rs:265-266` |
| **断言** | `result = 40` | `result == 40` | 测试通过 | `toy.rs:268` |

**运行时调用汇总**（在步骤 3 的 `JIT::default()` 中注册，在翻译阶段生成 `call` 指令，在执行阶段实际调用）：

| 调用顺序 | 运行时函数 | 来源 | 作用 |
|---|---|---|---|
| 1 | `dynamic_array_new_i64()` | `array.rs:5-9` | 创建空 DynamicArray |
| 2 | `dynamic_array_push_i64(arr, 10)` | `array.rs:13-20` | 压入 10 |
| 3 | `dynamic_array_push_i64(arr, 20)` | `array.rs:13-20` | 压入 20 |
| 4 | `dynamic_array_push_i64(arr, 30)` | `array.rs:13-20` | 压入 30 |
| 5 | `dynamic_array_push_i64(arr, 40)` | `array.rs:13-20` | 压入 40 |
| 6 | `dynamic_array_get_ptr_i64(arr, 3)` | `array.rs:45-55` | 获取第 3 个元素的指针 |
| 7 | `dynamic_array_drop_i64(arr)` | `array.rs:74-80` | 释放 DynamicArray |

---

## 第五章：附录

### 附录 A：类型映射表

| `frontend::Type` | `cranelift::types::Type` | 存储说明 |
|---|---|---|
| `I8` | `I8` | 8 位有符号整数 |
| `I16` | `I16` | 16 位有符号整数 |
| `I32` | `I32` | 32 位有符号整数 |
| `I64` | `I64` | 64 位有符号整数 |
| `I128` | `I128` | 128 位有符号整数（通过栈槽访问） |
| `F32` | `F32` | 32 位 IEEE 754 浮点数 |
| `F64` | `F64` | 64 位 IEEE 754 浮点数 |
| `String` | `I64` | 指向 null-terminated C 字符串的指针 |
| `Complex64` | `I64` | 打包：低 32 位 = 实部 (f32 bits)，高 32 位 = 虚部 (f32 bits) |
| `Complex128` | `I128` | 打包：低 64 位 = 实部 (f64 bits)，高 64 位 = 虚部 (f64 bits) |
| `Array(T, N)` | `I64` | 指向栈上数组的指针（调用外部函数时展开为 ptr+len） |
| `DynamicArray(T)` | `I64` | 指向堆上 `DynamicArray<T>` 结构体的指针 |

### 附录 B：运行时函数速查表

**IO 函数** (`src/runtime/io.rs`)：

| 函数签名 | 作用 |
|---|---|
| `toy_putchar(i64) -> i64` | 输出一个字符 |
| `toy_rand() -> i64` | 返回随机 i32（转为 i64） |
| `toy_sum_array(*const f64, usize) -> f64` | 对 f64 数组求和 |
| `toy_print_f64(f64) -> f64` | 打印 f64 并返回 |
| `toy_print_i64(i64) -> i64` | 打印 i64 并返回 |

**字符串函数** (`src/runtime/string.rs`)：

| 函数 | 来源 |
|---|---|
| `printf` | `libc::printf` |
| `puts` | `libc::puts` |

**数学函数** (`src/runtime/math.rs`)：

| 函数 | 对应 Rust 方法 |
|---|---|
| `toy_sin(f64) -> f64` | `f64::sin` |
| `toy_cos(f64) -> f64` | `f64::cos` |
| `toy_tan(f64) -> f64` | `f64::tan` |
| `toy_sqrt(f64) -> f64` | `f64::sqrt` |
| `toy_exp(f64) -> f64` | `f64::exp` |
| `toy_log(f64) -> f64` | `f64::ln` |
| `toy_ceil(f64) -> f64` | `f64::ceil` |
| `toy_floor(f64) -> f64` | `f64::floor` |
| `toy_pow(f64, f64) -> f64` | `f64::powf` |

**MKL 函数** (`src/runtime/mkl.rs`, 需 `--features mkl`)：

| 函数 | 作用 |
|---|---|
| `toy_mkl_dgemm(m, n, k, alpha, a_ptr, a_len, beta, b_ptr, b_len, c_ptr, c_len) -> i64` | 双精度矩阵乘法 C = alpha×A×B + beta×C（返回 0=成功, 负数=维度错误） |

### 附录 C：AST 节点速查表

| Expr 变体 | Toy 语法 | 说明 |
|---|---|---|
| `Literal(s, ty)` | `42`, `3.14` | 数字字面量 |
| `StringLiteral(s)` | `"hello"` | 字符串字面量 |
| `ComplexLiteral(re, im, ty)` | `1.5 + 2.5i` | 复数字面量 |
| `ArrayLiteral(elems, ty)` | `[1, 2, 3]` | 固定数组 |
| `DynamicArrayLiteral(elems, ty)` | `array [1, 2, 3]` | 动态数组 |
| `Identifier(name)` | `x` | 变量引用 |
| `Assign(name, expr)` | `x = 1` | 变量赋值 |
| `Add(lhs, rhs)` | `a + b` | 加法 |
| `Sub(lhs, rhs)` | `a - b` | 减法 |
| `Mul(lhs, rhs)` | `a * b` | 乘法 |
| `Div(lhs, rhs)` | `a / b` | 除法 |
| `Eq(lhs, rhs)` | `a == b` | 等于 |
| `Ne(lhs, rhs)` | `a != b` | 不等于 |
| `Lt(lhs, rhs)` | `a < b` | 小于 |
| `Le(lhs, rhs)` | `a <= b` | 小于等于 |
| `Gt(lhs, rhs)` | `a > b` | 大于 |
| `Ge(lhs, rhs)` | `a >= b` | 大于等于 |
| `IfElse(cond, then, else)` | `if cond { ... } else { ... }` | 条件分支 |
| `WhileLoop(cond, body)` | `while cond { ... }` | 循环 |
| `Call(name, args)` | `sin(x)`, `array_push(arr, 1)` | 函数调用 |
| `Index(base, idx)` | `arr[i]` | 数组索引 |
| `GlobalDataAddr(name)` | `&hello_string` | 全局数据地址 |
| `Cast(expr, ty)` | `x as f64` | 类型转换 |
| `Drop(name)` | `drop(arr)` | 显式释放 DynamicArray |

### 附录 D：新增功能修改清单

| 需求 | 需修改的文件 |
|---|---|
| 新增**语法**（如 for 循环） | ① `frontend.rs` — `Expr` 加变体 + PEG 加 rule ② `jit.rs` — `translate_expr` 加分支 ③ `type_checker.rs` — `infer_type` 加分支 ④ `optimizer.rs` — `fold_constants` 加分支（可选） ⑤ `ownership.rs` — `analyze_expr` 加分支（如果涉及 DynamicArray） |
| 新增**字面量类型**（如 bool） | ① `frontend.rs` — `Type` 加变体 + `Expr::Literal` 扩展 + PEG `literal()` rule ② `jit.rs` — `to_cranelift_type()` + `translate_expr` 的 `Literal` 分支 ③ `type_checker.rs` — `infer_type` |
| 新增**内置函数** | ① `runtime/` 对应模块或新文件 — 实现 `extern "C" fn` ② `runtime/registry.rs` — 注册 `builder.symbol(...)` ③ `type_checker.rs` — `register_builtins()` 加签名 ④ `jit.rs` — 如需特殊参数展开则修改 `translate_call` |
| 新增**优化 pass** | ① 新建 `src/my_pass.rs` ② `src/lib.rs` — 加 `pub mod my_pass` ③ `src/jit.rs` `compile()` — 插入调用 |
| 新增**静态检查** | ① 新建 `src/my_check.rs` ② `src/lib.rs` — 加 `pub mod my_check` ③ `src/jit.rs` `compile()` — 插入调用 |
| DynamicArray 支持**新元素类型** | ① `runtime/array.rs` — 加函数族（new/push/pop/len/cap/get_ptr/set/drop） ② `runtime/registry.rs` — 注册 ③ `type_checker.rs` — 加签名 + `infer_type` ④ `jit.rs` — `translate_dynamic_array_literal`/`translate_index`/auto-drop 加分支 ⑤ `ownership.rs` — `get_rhs_array_info` 加分支 |
