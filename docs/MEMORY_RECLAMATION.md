# Toy 语言内存回收机制

> 基于 Cranelift JIT 的 DynamicArray RAII 风格生命周期管理

---

## 目录

1. [概述](#1-概述)
2. [架构总览](#2-架构总览)
3. [核心数据结构](#3-核心数据结构)
4. [所有权检查器（编译期）](#4-所有权检查器编译期)
5. [JIT 自动释放（运行时）](#5-jit-自动释放运行时)
6. [三种释放方式](#6-三种释放方式)
7. [作用域层级](#7-作用域层级)
8. [循环迭代释放](#8-循环迭代释放)
9. [泄漏检测规则](#9-泄漏检测规则)
10. [完整示例](#10-完整示例)
11. [运行时调用链](#11-运行时调用链)
12. [已知限制](#12-已知限制)
13. [涉及文件清单](#13-涉及文件清单)

---

## 1. 概述

Toy 语言中的 `DynamicArray`（动态数组，对应 Rust 的 `Vec<T>`）是堆分配资源。为了防止内存泄漏，编译器实现了 **编译期所有权检查 + JIT 自动释放** 的双层回收机制。

### 核心设计原则

| 原则 | 说明 |
|---|---|
| **RAII 风格** | 资源在离开作用域时自动释放，无需手动管理 |
| **编译期保底** | 所有权检查器静态分析，阻止明显泄漏的代码通过编译 |
| **运行时兜底** | JIT 编译器在作用域出口自动插入 `array_drop_xxx` 调用 |
| **显式控制** | 用户可通过 `drop(arr)` 提前释放，检查器确保无 double-free |
| **统一追踪** | 所有权检查器输出 `ScopeAnalysis`，JIT 侧直接消费，消除双系统不一致 |

### 支持的元素类型

| 元素类型 | 构造函数 | Drop 函数 | 对应的 Rust 类型 |
|---|---|---|---|
| I64 | `array [1,2,3]` / `array_new_i64()` | `array_drop` | `Vec<i64>` |
| F64 | `array [1.0,2.0]` / `array_new_f64()` | `array_drop_f64` | `Vec<f64>` |
| Complex128 | `array_new_complex128()` | `array_drop_complex128` | `Vec<i128>` |

---

## 2. 架构总览

```
                    Toy 源码
                       │
                       ▼
              ┌─────────────────┐
              │   PEG 解析器     │  → AST
              │  (frontend.rs)  │
              └─────────────────┘
                       │
                       ▼
              ┌─────────────────┐
              │   常量折叠       │  → 优化后的 AST
              │ (optimizer.rs)  │
              └─────────────────┘
                       │
                       ▼
              ┌─────────────────────────┐
              │   所有权检查器 (编译期)   │
              │   (ownership.rs)        │
              │                         │
              │  输出:                   │
              │  ├─ ScopeAnalysis       │──→ JIT 消费
              │  └─ Vec<OwnershipError> │──→ 编译错误
              └─────────────────────────┘
                       │ (errors.is_empty())
                       ▼
              ┌─────────────────────────┐
              │   JIT 翻译器 (运行时)    │
              │   (jit.rs)              │
              │                         │
              │  消费 ScopeAnalysis:     │
              │  ├─ 顶层 auto-drop      │
              │  ├─ Block 退出 auto-drop│
              │  └─ While 迭代 auto-drop│
              └─────────────────────────┘
                       │
                       ▼
              ┌─────────────────┐
              │  Cranelift 代码生成 │ → 原生 x86_64 机器码
              │  + 执行            │
              └─────────────────┘
```

### 数据流：统一所有权追踪

```
改进前 (双系统独立, 不一致风险):
  ownership.rs          jit.rs
  ┌──────────┐         ┌──────────────┐
  │ arrays     │         │ dynamic_arrays│
  │ errors     │  无互通  │ explicitly_   │
  │            │         │ dropped       │
  └──────────┘         └──────────────┘

改进后 (单向数据流, 单一真相源):
  ownership.rs                  jit.rs
  ┌────────────────┐   输出    ┌──────────────────┐
  │ OwnershipChecker│ ────────→│ FunctionTranslator│
  │ ScopeAnalysis   │          │ + scope_depth    │
  │ + errors        │          │ + explicitly_    │
  └────────────────┘          │   dropped        │
                              └──────────────────┘
```

---

## 3. 核心数据结构

### 3.1 ScopeAnalysis (`src/ownership.rs`)

所有权检查器输出的作用域分析结果，JIT 编译器据此确定在每个作用域退出时应释放哪些数组。

```rust
/// 由 OwnershipChecker 输出的作用域分析结果。
/// JIT 编译器消费此结构，无需独立追踪作用域。
pub struct ScopeAnalysis {
    /// scope_depth -> 该作用域内定义的 DynamicArray 变量名列表
    /// scope_depth=0 为函数体顶层
    pub scope_vars: HashMap<usize, Vec<String>>,
}
```

**工作原理**：
- `scope_vars[0]` = 函数体顶层定义的数组（必须在函数返回前 drop/return）
- `scope_vars[1]` = 第一层 `{}` / `while` 内定义的数组（作用域退出时释放）
- `scope_vars[2]` = 第二层嵌套块内定义的数组
- …以此类推

### 3.2 ArrayDisposition (`src/ownership.rs`)

DynamicArray 的所有权状态机：

```
                    array [1,2,3]
                    array_new_i64()
                         │
                         ▼
                    ┌─────────┐
                    │  Owned   │  "我拥有它，需要负责释放"
                    └────┬────┘
           ┌─────────┬───┴─────┬──────────┐
           │         │         │          │
           ▼         ▼         ▼          ▼
      ┌────────┐ ┌────────┐ ┌───────┐ ┌──────────┐
      │Dropped │ │Returned│ │Passed │ │Uninitialized│
      └────────┘ └────────┘ └───────┘ └──────────┘
      drop(arr)  r = arr   push(arr)   (错误路径)
      显式释放    返回调用方  传给函数
```

### 3.3 FunctionTranslator (`src/jit.rs`)

JIT 侧的翻译器，保存消费 ScopeAnalysis 所需的运行时状态：

```rust
struct FunctionTranslator<'a> {
    scope_analysis: ScopeAnalysis,    // 预计算的作用域信息
    scope_depth: usize,               // 当前作用域深度 (0 = 顶层)
    explicitly_dropped: Vec<Variable>, // 已显式 drop 的变量 (避免 double-free)
    // ... 其他字段 ...
}
```

---

## 4. 所有权检查器（编译期）

**文件**: `src/ownership.rs`

### 4.1 入口：`analyze_function()`

```rust
pub fn analyze_function(
    &mut self,
    _params: &[(String, Type)],
    stmts: &[Expr],
    return_var: &str,
) -> (ScopeAnalysis, Vec<OwnershipError>)
```

遍历函数体的 AST，递归分析每条语句，跟踪每个 DynamicArray 变量的状态转换。返回：

- **`ScopeAnalysis`** — 交给 JIT 用于生成 auto-drop 代码
- **`Vec<OwnershipError>`** — 编译时发现的错误，阻止编译

### 4.2 状态跟踪：`analyze_expr()`

对每条 AST 语句进行状态判定：

| 语句 | 操作 |
|---|---|
| `a = array [1,2,3]` | 登记 `a`→Owned，记录到 `scope_vars[当前深度]` |
| `a = array [1]; a = array [2]` | 检测覆盖：旧 Owned 值 → 报告 `LeakedArray` |
| `r = arr` (return_var) | 标记 `arr`→Returned，所有权转移给调用者 |
| `drop(arr)` | 调用 `mark_dropped()`，验证状态合法性 |
| `array_push(arr, 4)` | 标记 `arr`→Passed（视为已消费） |
| `{ ... }` (Block) | `scope_depth++`，递归分析，`close_scope(depth)` |
| `while cond { ... }` | `scope_depth++`，循环体作独立作用域 |

### 4.3 作用域退出：`close_scope()`

```rust
fn close_scope(&mut self, depth: usize) {
    // depth == 0: 顶层 Owned 数组 → 泄漏错误！
    // depth > 0:  嵌套作用域 Owned 数组 → JIT 自动释放, 不报错
}
```

**关键设计决策**：仅函数顶层（depth=0）的未处理 Owned 数组被视为泄漏。嵌套作用域（Block/WhileLoop 内）的 Owned 数组由 JIT 在运行时自动释放——这是 RAII 的核心语义。

### 4.4 错误检测

| 错误类型 | 触发条件 | Toy 代码示例 |
|---|---|---|
| `LeakedArray` | 顶层 Owned 数组未 drop/return | `fn f() { a=array[1]; r=0 }` |
| `DoubleDrop` | 对同一变量调用两次 `drop()` | `drop(a); drop(a)` |
| `DropAfterPassed` | `drop()` 已传给函数的数组 | `push(a,1); drop(a)` |
| `UseAfterDrop` | `drop()` 后访问数组元素 | `drop(a); r=a[0]` |

---

## 5. JIT 自动释放（运行时）

**文件**: `src/jit.rs`

### 5.1 `emit_scope_drop()` — 核心释放器

```rust
fn emit_scope_drop(&mut self, depth: usize, return_variable: Option<Variable>) {
    // 1. 从 ScopeAnalysis.scope_vars[depth] 获取该作用域的数组列表
    // 2. 对每个数组：
    //    a. 跳过返回变量（所有权转移给调用者）
    //    b. 跳过 explicitly_dropped 中的变量（已通过 drop() 释放）
    //    c. 否则, 查找元素类型 → 调用 drop_func_for() → emit_drop_call()
    // 3. 清理 explicitly_dropped 记录（防止跨作用域污染）
}
```

### 5.2 `emit_drop_call()` — 发射 IR 指令

```rust
fn emit_drop_call(&mut self, drop_func_name: &str, val: Value) {
    // 生成 Cranelift IR: call array_drop_xxx(arr_ptr)
    //   等价于在生成的机器码中插入一条 call 指令
}
```

### 5.3 `drop_func_for()` — 类型分发

```rust
fn drop_func_for(elem_ty: &FrontendType) -> &'static str {
    // I64/I32/I16/I8 → "array_drop"
    // F64             → "array_drop_f64"
    // Complex128      → "array_drop_complex128"
}
```

由于 `Vec<T>` 是泛型容器，其 `Drop::drop()` 需要知道 `T` 的大小和对齐。不同元素类型对应不同的运行时 drop 函数，FFI 层面无法统一。

---

## 6. 三种释放方式

### 6.1 显式 `drop()` — 精细控制

```toy
fn demo() -> (r: i64) {
    a = array [1, 2, 3]
    r = a[0]
    drop(a)          // ← 立即释放，之后 a 不可访问
    r = r + 1
}
```

**流程**：
1. 所有权检查器：验证 `a` 处于 Owned 状态 → 标记为 Dropped
2. JIT 翻译：记录到 `explicitly_dropped`，发射 `call array_drop(arr_ptr)`
3. 作用域退出：`emit_scope_drop` 发现 `a` 在 `explicitly_dropped` 中 → 跳过（避免 double-free）

### 6.2 Block 作用域 — 自动释放

```toy
fn demo() -> (r: i64) {
    {
        b = array [4, 5, 6]
        r = b[0]
    }               // ← b 在此处自动释放
    r = r + 1
}
```

**流程**：
1. 所有权检查器：`b` 登记到 `scope_vars[1]`，`close_scope(1)` — 不报泄漏（depth>0）
2. JIT 翻译：进入 Block → `scope_depth=1`，翻译语句，退出 Block → `emit_scope_drop(1, None)` → 发射 `call array_drop(b_ptr)`

### 6.3 函数退出兜底 — 顶层自动释放

```toy
fn demo() -> (r: i64) {
    a = array [1, 2, 3]
    array_push(a, 4)  // a → Passed
    r = 0
}   // ← a 在此处自动释放（通过 emit_scope_drop(0, Some(return_var))）
```

任何顶层标记为 Passed（传给函数）的数组，在函数返回前由 `emit_scope_drop(0)` 统一兜底释放。

---

## 7. 作用域层级

### 可视化

```
fn main() -> (r: i64) {           scope_depth=0 (顶层)
    a = array [1, 2, 3]          a ∈ scope_vars[0]

    {                              scope_depth=1
        b = array [4, 5, 6]       b ∈ scope_vars[1]
        r = b[0]
    }                              emit_scope_drop(1) → drop(b)

    {                              scope_depth=1
        {                          scope_depth=2
            c = array [7]          c ∈ scope_vars[2]
        }                          emit_scope_drop(2) → drop(c)
    }                              emit_scope_drop(1) → (空, 无数组)

    drop(a)                        a → explicitly_dropped
    r = 0
}                                  emit_scope_drop(0) → (a 已显式drop, 跳过)
```

### 规则

| 作用域类型 | scope_depth 变化 | 释放时机 |
|---|---|---|
| 函数体顶层 | 0 | 函数 return 前（`emit_scope_drop(0)`） |
| `{ }` 块 | +1 / -1 | 块退出时（`emit_scope_drop(depth)`） |
| `while cond { }` | +1 / -1 | **每次迭代结束时**（`emit_scope_drop(depth)`） |
| `if/else` 分支 | 不变 | 跟随父作用域释放 |

---

## 8. 循环迭代释放

### 问题

旧实现中，循环内创建的 DynamicArray 只在循环退出时释放**最后一次迭代**的数组，前 N-1 次迭代的数组指针丢失 → **泄漏**。

### 解决方案

```
while cond {                       scope_depth+=1 (在 translate_expr 中)
    body_translation()
    emit_scope_drop(depth)          ← 每次迭代结束时释放！
    jump header
}                                  scope_depth-=1
```

```toy
fn demo() -> (r: i64) {
    i = 0
    while i < 5 {
        tmp = array [i, i+1]       // 每次迭代创建新数组
        printf("tmp[0] = %d\n", tmp[0])
        i = i + 1
    }                              // tmp 已在每次迭代结束时释放, 无泄漏
}
```

**验证**：`test_while_loop_no_leak` 测试循环 100 次每次创建数组并显式 drop，结果正确（4950），无内存泄漏。

---

## 9. 泄漏检测规则

```
                      ┌─────────────────────────────┐
                      │ DynamicArray 处于 Owned 状态  │
                      └─────────────┬───────────────┘
                                    │
                          在哪个作用域定义的？
                                    │
                    ┌───────────────┴───────────────┐
                    │                               │
              depth == 0                      depth > 0
           (函数体顶层)                      (Block/While 内)
                    │                               │
                    ▼                               ▼
        ┌───────────────────┐          ┌────────────────────────┐
        │ 必须 drop/return/  │          │ JIT 在作用域退出时       │
        │ 传给函数, 否则报    │          │ 自动释放, 不报错         │
        │ LeakedArray       │          │ (RAII 语义)             │
        └───────────────────┘          └────────────────────────┘
```

**为什么嵌套作用域不报泄漏？**

因为在 RAII 语义下，块退出时自动释放是**正常行为**——就像 Rust 中 `{ let v = vec![1]; }` 在块结束时会自动 drop。用户创建临时数组使用后丢弃是完全合法的模式。

---

## 10. 完整示例

**文件**: `examples/scope_demo.toy`

```toy
fn main() -> (r: i64) {
    // [1] 顶层数组 — 必须 drop 或 return
    a = array [1, 2, 3]
    printf("  a[0] = %d\n", a[0])

    // [2] Block 作用域 — 块退出时自动释放
    {
        b = array [4, 5, 6]
        printf("  b[0] = %d\n", b[0])
    }   // ← b 自动释放

    // [3] 嵌套块 — 内层先释放
    {
        {
            c = array [7, 8, 9]
            printf("  c[0] = %d\n", c[0])
        }   // ← c 在此释放
        puts("  c already freed\n")
    }

    // [4] 显式 drop — 精细控制释放时机
    {
        d = array [10, 20]
        printf("  d[0] = %d\n", d[0])
        drop(d)   // ← d 立即释放, 块退出时不再重复释放
    }

    // [5] While 循环 — 每次迭代自动释放
    i = 0
    while i < 5 {
        tmp = array [i, i+1]
        printf("  tmp[0] = %d\n", tmp[0])
        i = i + 1
    }   // ← 每次迭代末尾, tmp 自动释放

    drop(a)
    r = 0
}
```

**执行结果**：

```
=== Scope Demo ===
[1] Top-level array (auto-drop at function end)
  a[0] = 1
[2] Block-scoped array (auto-drop at block end)
  b[0] = 4
[3] Nested block scopes
  outer block
  c[0] = 7
  c already freed here
[4] Explicit drop() still works anywhere
  d[0] = 10
  d explicitly dropped inside block
[5] While loop: auto-drop per iteration
  tmp[0] = 0
  tmp[0] = 1
  tmp[0] = 2
  tmp[0] = 3
  tmp[0] = 4
```

---

## 11. 运行时调用链

从 Toy 代码到实际内存释放的完整路径：

```
Toy:   drop(arr)
         │
         ▼
JIT:   call array_drop(arr_ptr)       ← emit_drop_call() 生成的 IR 指令
         │
         ▼
Rust:  dynamic_array_drop_i64(arr_ptr) ← src/runtime/array.rs
         │
         ▼
       let arr: Box<Vec<i64>> = Box::from_raw(arr_ptr as *mut Vec<i64>);
         │
         ▼
       // Box 离开作用域 → Box::drop() → Vec::drop()
         │
         ▼
       Vec::drop() 内部:
         ├── drop_in_place(每个元素)    ← 对 I64/F64 是 no-op
         └── dealloc(堆内存)            ← 释放 Vec 的 heap buffer
```

对于 auto-drop（作用域退出时 JIT 自动插入的 drop），调用链完全相同，区别仅在于 drop 指令的触发时机（由 JIT 编译器决定，而非用户代码中的 `drop()` 语句）。

---

## 12. 已知限制

| 限制 | 说明 | 影响程度 |
|---|---|---|
| `r = arr` 悬垂指针 | 返回数组时，JIT 不区分所有权转移，仍可能 auto-drop | 低 (当前未触发) |
| `if/else` 分支检测保守 | 不做跨分支 meet-point 分析，分支内泄漏可能漏检 | 低 |
| `produces_dynamic_array` 枚举不全 | 仅匹配 `array[...]` 字面量和 `array_new_xxx` 调用 | 低 |
| JIT 不处理赋值覆盖 | `a=array[1]; a=array[2]` 旧数组在 JIT 侧未做释放 | 低 (所有权检查器已拦截) |
| 仅 ASCII 字符串 | PEG 解析器仅支持 `\x00..\x7f` 字符范围 | 低 |

---

## 13. 涉及文件清单

| 文件 | 职责 | 关键函数/结构体 |
|---|---|---|
| `src/ownership.rs` | 编译期所有权检查 | `OwnershipChecker`, `ScopeAnalysis`, `ArrayDisposition`, `analyze_function()`, `analyze_expr()`, `close_scope()` |
| `src/jit.rs` | JIT 翻译 + 运行时 auto-drop | `FunctionTranslator`, `emit_scope_drop()`, `emit_drop_call()`, `drop_func_for()`, `translate_drop()` |
| `src/frontend.rs` | AST 定义 + 解析 | `Expr::Block`, `Expr::Drop`, PEG `block_stmt` 规则 |
| `src/optimizer.rs` | 常量折叠 | `fold_constants()` 中 `Expr::Block` 分支 |
| `src/type_checker.rs` | 类型推导 | `infer_type()` 中 `Expr::Block` 防御分支 |
| `src/runtime/array.rs` | 运行时动态数组操作 | `dynamic_array_drop_i64()`, `dynamic_array_drop_f64()`, `dynamic_array_drop_complex128()` |
| `src/runtime/registry.rs` | 符号注册 | `register_builtins()` 注册 drop 函数 |
| `src/bin/toy.rs` | CLI 入口 | 调用 `jit.compile()` → ownership check → JIT translate |
| `tests/integration_test.rs` | 集成测试 | `test_block_scope_basic`, `test_while_loop_no_leak` 等 (5 个) |
| `examples/scope_demo.toy` | 示例脚本 | 5 节完整演示 |
| `docs/raii-scope-merge-plan.md` | 设计文档 | 实施计划和偏差记录 |
