# Cranelift JIT Demo — Toy 语言即时编译器

基于 [Cranelift](https://github.com/bytecodealliance/wasmtime/tree/main/cranelift) 代码生成后端的类 C 脚本语言 JIT 编译器。代码被即时翻译为 x86_64 原生机器码并在进程内直接执行，无需解释器循环。

在原 [bytecodealliance/cranelift-jit-demo](https://github.com/bytecodealliance/cranelift-jit-demo) 基础上新增了完整类型系统、所有权检查、RAII 风格内存管理、动态数组、复数运算和 Intel MKL 集成。

---

## 快速开始

```bash
# 构建（单次）
cargo build --release

# 运行 .toy 脚本
target/release/toy.exe examples/all_features.toy

# 运行内置测试
target/release/toy.exe --test
```

生成的 `toy.exe` 是自包含的独立可执行文件，复制到任意 Windows 机器即可使用，无需安装 Rust 或任何运行时。

---

## 语言概览

```rust
fn fib(n: i64) -> (r: i64) {
    if n < 2 {
        r = n
    } else {
        a = fib(n - 1)
        b = fib(n - 2)
        r = a + b
    }
}
```

### 支持的特性

| 类别 | 特性 |
|---|---|
| **类型** | i8 / i16 / i32 / i64 / i128 / f32 / f64 / string / complex64 / complex128 |
| **容器** | 固定数组 `[1, 2, 3]` / 动态数组 `array [1, 2, 3]` |
| **控制流** | `if`-`else` / `while` 循环 / 块作用域 `{ }` |
| **运算符** | `+` `-` `*` `/` / `==` `!=` `<` `<=` `>` `>=` / `as` 类型转换 |
| **函数** | 递归调用 / 内置数学函数 (`sin`, `cos`, `pow`, `sqrt`, `log` 等) |
| **I/O** | `printf` / `puts` / `putchar` / `print_f64` / `print_i64` / `rand` |
| **内存管理** | 显式 `drop()` / RAII 块作用域自动释放 / 编译期所有权检查 |
| **线性代数** | Intel MKL `cblas_dgemm` 矩阵乘法（可选 feature） |

全特性示例见 [`examples/all_features.toy`](examples/all_features.toy)。

---

## 构建 & 测试

```bash
# Debug 构建
cargo build

# Release 构建（~5.6 MB，推荐分发用）
cargo build --release

# 启用 Intel MKL 支持
cargo build --release --features mkl

# 运行 .toy 脚本
target/release/toy examples/sin.toy

# 运行所有测试（32 个）
cargo test

# 运行基准测试
cargo bench

# 代码检查
cargo clippy
```

---

## 编译管线

```
.toy 源码
  │
  ▼
┌──────────────┐
│  PEG 解析器   │  → AST（26 种表达式 + 11 种类型）
└──────────────┘
  │
  ▼
┌──────────────┐
│  常量折叠     │  → 编译期求值、算术恒等式消除
└──────────────┘
  │
  ▼
┌──────────────┐
│  所有权检查   │  → ScopeAnalysis + 泄漏/DoubleDrop/UseAfterDrop 检测
└──────────────┘
  │
  ▼
┌──────────────┐
│  JIT 翻译     │  → AST → Cranelift IR (SSA)，自动插入 array_drop 调用
└──────────────┘
  │
  ▼
┌──────────────┐
│  Cranelift    │  → IR → x86_64 机器码，写入可执行内存页
│  代码生成     │
└──────────────┘
  │
  ▼
  mem::transmute → extern "C" fn → call → 执行
```

---

## 内存管理（RAII 风格）

DynamicArray 是堆分配资源。编译器实现了 **编译期检查 + 运行时自动释放** 的双层回收机制。

| 方式 | 说明 | 示例 |
|---|---|---|
| **显式 `drop()`** | 手动释放，之后不可访问 | `drop(arr)` |
| **块作用域自动** | `{ }` 退出时自动释放块内数组 | `{ a = array[1]; }` |
| **循环迭代释放** | `while` 每次迭代结束释放循环体数组 | `while i<5 { tmp = array[i]; i=i+1 }` |
| **函数退出兜底** | 顶层未处理的数组在 return 前自动释放 | `push(arr,1)` 后不再手动管理 |

### 所有权检查

编译期静态分析拦截以下错误：

```rust
// 泄漏 — 顶层数组未 drop/return
fn leak() -> (r: i64) { a = array[1]; r = 0 }

// 重复释放
fn dd() -> (r: i64) { a = array[1]; drop(a); drop(a); r = 0 }

// 释放后使用
fn uad() -> (r: i64) { a = array[1]; drop(a); r = a[0] }

// 覆盖旧值
fn o() -> (r: i64) { a = array[1]; a = array[2]; r = 0 }
```

详见 [`docs/MEMORY_RECLAMATION.md`](docs/MEMORY_RECLAMATION.md)。

---

## 分发包

只需两个文件即可在任意 Windows 机器上运行：

```
toy.exe           ← Release 构建产物 (~5.6 MB，自包含)
demo.toy          ← Toy 脚本
```

```bash
toy.exe demo.toy
```

不需要 Rust、Cargo 或任何外部运行时。

---

## 示例脚本

| 文件 | 说明 |
|---|---|
| [`examples/all_features.toy`](examples/all_features.toy) | 18 节全特性演示 |
| [`examples/scope_demo.toy`](examples/scope_demo.toy) | RAII 块作用域 + 循环释放演示 |
| [`examples/ownership_demo.toy`](examples/ownership_demo.toy) | 所有权系统正常用例 |
| [`examples/ownership_errors.toy`](examples/ownership_errors.toy) | 触发所有错误类型（不通过编译） |
| [`examples/array_basic.toy`](examples/array_basic.toy) | 动态数组基础操作 |
| [`examples/array_iteration.toy`](examples/array_iteration.toy) | while 遍历动态数组 |
| [`examples/array_resize.toy`](examples/array_resize.toy) | 动态数组扩容 |
| [`examples/sin.toy` / `cos.toy`](examples/) | 数学函数最小示例 |
| [`examples/matrix_mkl.toy`](examples/matrix_mkl.toy) | MKL DGEMM 矩阵乘法（需 `--features mkl`） |

---

## 项目结构

```
src/
  frontend.rs       PEG 解析器 + AST 定义 (26 种 Expr, 11 种 Type)
  jit.rs            Cranelift JIT 编译器 + auto-drop 运行时释放
  optimizer.rs      常量折叠优化 pass
  ownership.rs      所有权检查器 + ScopeAnalysis 输出
  type_checker.rs   类型推导 + 内置函数签名注册
  runtime/
    array.rs        动态数组运行时 (Vec<T> 的 C ABI 包装)
    io.rs           输入输出 (printf, puts, rand, putchar)
    math.rs         数学库 (sin, cos, pow, sqrt, exp, log 等)
    mkl.rs          Intel MKL cblas_dgemm FFI 绑定
    registry.rs     JIT 符号注册表
    string.rs       字符串 (printf/puts 重导出)
  cli/mod.rs        CLI 参数解析 (clap derive)
  bin/toy.rs        main 入口 (--test / 脚本路径)
  lib.rs            crate 根

raii_demo/          手写 RAII DynamicArray 容器 (参考实现)
benches/            Criterion 性能基准 (JIT vs 原生 Rust)
tests/              集成测试 + 类型检查器测试
examples/           .toy 示例脚本
docs/
  MEMORY_RECLAMATION.md      内存回收机制详细文档
  raii-scope-merge-plan.md    RAII 实施方案与偏差记录
```

---

## 性能

JIT 编译的 Toy 代码与原生 Rust 性能对比：

| 场景 | JIT | 原生 Rust | 开销 |
|---|---|---|---|
| `sin(x)` 调用 | 9.55 ns | 9.20 ns | ~3.8% |
| 8 元素数组求和 | 6.02 ns | — | — |
| 110 次 dynamic push | 1.17 μs | 697 ns | ~1.7× (FFI 边界) |

数学运算上 JIT 与原生几乎无差距；动态数组的差距主要来自跨 FFI 调用运行时函数。

```bash
# 运行基准
cargo bench --bench jit_bench
```

---

## 技术栈

| 组件 | 版本 |
|---|---|
| Rust Edition | 2024 |
| Cranelift | 0.125 |
| PEG 解析器 | 0.8 |
| Clap CLI | 4.5 |
| Criterion 基准 | 0.8 |
| Intel MKL (可选) | 0.8 |
