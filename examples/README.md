# Toy 语言示例集

本目录是 Toy 语言的 `.toy` 源文件示例，配合 Cranelift JIT 编译运行。推荐从 `all_features.toy` 开始，它覆盖了语言的全部主要特性。

---

## 快速开始

```bash
# 在项目根目录
cargo run -- examples/all_features.toy
```

**预期结果：** 程序打印 18 个分节的输出（每节以 `[N] xxx` 开头），最后一行是 `Return value: 0`，进程退出码为 0。

**运行其他示例：**

```bash
cargo run -- examples/sin.toy           # sin(π/2)
cargo run -- examples/array_resize.toy  # 动态数组 push 演示
cargo run -- examples/matrix_mkl.toy    # MKL 矩阵乘法 (需 mkl feature)
```

**MKL feature：**

```bash
cargo run --features mkl -- examples/matrix_mkl.toy
```

---

## `all_features.toy` 详解

整个脚本是一个单文件 `main` 函数（toy 语法当前只支持一个函数/文件），通过 `puts()` 打印分节标题，演示 18 类特性。下表列出每节演示的功能、对应的源码位置、以及"追踪"时的关键看点。

| 节 | 演示内容 | 关键源码 | 追踪要点 |
|---|---|---|---|
| **[1]** | 整数 `+ - * /` | [src/jit.rs:326](../src/jit.rs#L326) `translate_binary_op` | 看 `Expr::Add/Sub/Mul/Div` 如何映射到 Cranelift IR 的 `iadd/isub/imul/udiv` |
| **[2]** | 类型转换链 `i32→i64→i128→i64`、`f32↔f64` | [src/jit.rs:492](../src/jit.rs#L492) `translate_cast` | `sextend / ireduce / fpromote / fdemote / fcvt_*` |
| **[3]** | 浮点四则 | [src/jit.rs:333](../src/jit.rs#L333) | 浮点分支的 `fadd/fsub/fmul/fdiv` |
| **[4]** | `== != < <= > >=` | [src/jit.rs:528](../src/jit.rs#L528) `translate_cmp` | 整数 `icmp` + `select` 拼出 0/1 |
| **[5]** | 嵌套 `if-else` | [src/jit.rs:612](../src/jit.rs#L612) `translate_if_else` | `brif / jump / merge_block / BlockArg::Value` |
| **[6]** | `while` 求和 | [src/jit.rs:667](../src/jit.rs#L667) `translate_while_loop` | `header_block / body_block / exit_block` 三个基本块 |
| **[7]** | `while` + `printf` | 同 [6](../src/jit.rs#L667) | 循环内外部函数调用的栈布局 |
| **[8]** | 固定数组 `[i64; 5]` 索引 | [src/jit.rs:823](../src/jit.rs#L823) `translate_array_literal` | 数组在栈槽 (`StackSlot`) 上的存储布局 |
| **[9]** | `while` 遍历数组求和 | [src/jit.rs:920](../src/jit.rs#L920) `translate_index` | 边界检查 `icmp + trapnz` |
| **[10]** | f64 固定数组 + `toy_sum_array` | [src/runtime/array.rs:18](../src/runtime/array.rs#L18) `toy_sum_array` | 固定数组作为外部函数时如何展开为 `(ptr, len)` 两参数 |
| **[11]** | 动态数组 `array_push/len/索引` | [src/runtime/array.rs:6](../src/runtime/array.rs#L6) `dynamic_array_new_i64` | `Box::new(DynamicArray::new())` → `Box::into_raw` |
| **[12]** | `array_set / array_pop` | [src/runtime/array.rs:60,23](../src/runtime/array.rs#L60) | 索引越界返回 -1；pop 弹空返回 0 |
| **[13]** | `drop()` (可选，自动释放) | [src/ownership.rs](../src/ownership.rs) (静态检查) | 任何传给函数的动态数组会在函数返回前由 jit.rs 兜底释放 |
| **[14]** | 复数 `+ - * /` | [src/jit.rs:998](../src/jit.rs#L998) `translate_complex_binop` | Complex128 用 16 字节栈槽打包两个 f64 |
| **[15]** | `sin/cos/tan/sqrt/pow/log/exp/ceil/floor` | [src/runtime/math.rs](../src/runtime/math.rs) | 调用 `libm` 的 `sin/cos/...` |
| **[16]** | `puts / printf` 字符串 | [src/runtime/string.rs:3](../src/runtime/string.rs#L3) | 直接 re-export libc 的 `puts/printf` |
| **[17]** | `putchar` 字符输出 | [src/runtime/io.rs:6](../src/runtime/io.rs#L6) | 每次写一个字节到 stdout |
| **[18]** | `rand()` 随机数 | [src/runtime/io.rs:12](../src/runtime/io.rs#L12) | 用 `rand::rng().random::<i32>()` |

### 输出顺序

脚本运行时**先打印每节的标题**（来自 `puts()`），**再打印该节内的实际结果**（来自 `print_f64/printf`）。例如节 [1]：

```
[1] Basic Arithmetic (i64) - src/jit.rs:326 translate_binary_op
a + b = 13
a - b = 7
a * b = 30
a / b = 3
```

---

## 端到端的"追踪"路径

要理解 `cargo run -- examples/all_features.toy` 背后发生了什么，按下面顺序看代码：

1. **[src/cli/mod.rs](../src/cli/mod.rs)** — `Cli::parse_args()` 解析命令行，把文件路径交给 `main`。
2. **[src/bin/toy.rs:26](../src/bin/toy.rs#L26) `run_script`** — 读文件、初始化 JIT、调 `jit.compile()`、`mem::transmute` 转函数指针、执行。
3. **[src/jit.rs:66](../src/jit.rs#L66) `JIT::compile`** — 编译入口：
   - `parser::function()` 解析源文件 → AST ([src/frontend.rs:62](../src/frontend.rs#L62))
   - `optimizer::fold_constants_in_stmts` 常量折叠
   - `ownership::OwnershipChecker::analyze_function` 静态所有权检查
   - `self.translate()` AST → Cranelift IR
   - `module.define_function` IR → 机器码
4. **[src/jit.rs:66 `JIT::default`](../src/jit.rs#L37)** — JIT 初始化：探测 CPU ISA → 创建 `JITModule` → `runtime::register_builtins` 注册全部内置函数符号。
5. **[src/runtime/registry.rs:17](../src/runtime/registry.rs#L17) `register_builtins`** — `printf/puts/array_*/math_*` 等函数以 `extern "C"` 形式注册到 JIT。
6. **[src/ownership.rs](../src/ownership.rs)** — 编译期追踪每个 `DynamicArray` 变量的状态：未初始化 / 已拥有 / 已返回 / 已释放 / 已转交。
7. **[src/runtime/array.rs](../src/runtime/array.rs)** — `DynamicArray<T>` 的 C ABI 包装，`Box::into_raw` ↔ `Box::from_raw` 桥接 Rust 所有权。
8. **[memory_bench/raii_demo/src/lib.rs](../memory_bench/raii_demo/src/lib.rs)** — 手写 RAII 容器，直接调 `std::alloc::alloc/realloc/dealloc` 和 `std::ptr::*`。

---

## 已知 Bug 与本脚本的绕过方式

在编写本脚本时，发现所有权检查器和类型推断的若干 bug。脚本已用绕过方式处理，**不影响演示，但源码里有这些坑需要注意：**

### Bug 1（已修复）：`ownership.rs` 的 `Call` 分支不再区分"借用"与"消费"

**修复前现象：** 一旦写过 `array_push(darr, 100)`，必须显式 `drop(darr)`，否则 checker 报 `array 'darr' is leaked`，导致 `cargo run -- --test` 跑不过、`examples/array_*.toy` 跑不过。

**修复方式：** 移除 [src/ownership.rs](../src/ownership.rs) 中 `Expr::Call` 分支里的 `non_consuming` 白名单。任何把 DynamicArray 作为参数传入的函数调用都直接标记为 `Passed`，泄漏检查立即放行，运行时由 [src/jit.rs:196-231](../src/jit.rs#L196) 的 `dynamic_arrays` 兜底释放。

**新规则：**
- 动态数组只要**传给过任何函数**（如 `array_push/array_len/...`），所有权视为已转交，**不能再显式 `drop()`**，否则会报 `DropAfterPassed` 错误。
- 想要"立即释放"？把 `drop(darr)` 那行直接删掉即可——函数返回前 jit.rs 会自动释放。
- 这一改动也影响 `mark_dropped`：Passed 状态下 drop 会触发新的 `OwnershipError::DropAfterPassed` 变体，错误消息会引导用户直接删除 `drop()` 调用。

### Bug 2：`type_checker::infer_type` 把 `toy_sum_array` 的返回类型推断为 `I64`

**现象：** `fsum = toy_sum_array(farr)` 后 `fsum` 实际被认作 `I64`，传给 `print_f64` 时签名冲突。

**位置：** [src/type_checker.rs:333](../src/type_checker.rs#L333) — `"toy_sum_array" => Type::I64`，应该是 `Type::F64`。

**绕过：** 脚本第 [10] 节写 `fsum = toy_sum_array(farr) as f64`，显式转换。

### Bug 3：固定数组和动态数组的 `array_len` 签名冲突

**现象：** `array_len(fixed_array)` 展开为 `(ptr, len)` 两参数；`array_len(dyn_array)` 只有 `(ptr)` 一参数。同一模块里两种调用会让 Cranelift 报 `IncompatibleSignature`。

**位置：** [src/jit.rs:709](../src/jit.rs#L709) 的 `should_expand` 逻辑 + [src/runtime/array.rs:31](../src/runtime/array.rs#L31) 动态数组版本的 `array_len`。

**绕过：** 脚本第 [8] 节（固定数组）直接用常量 `5` 当长度，不调 `array_len`；只在第 [11] 节（动态数组）里调 `array_len`。

### Bug 4：`printf` 是变参函数，相同名字不同 arity 会被 Cranelift 拒绝

**现象：** `printf("a + b = %d\n", x)` 和 `printf("%s = %d\n", name, num)` 两次调用参数数量不同，Cranelift 视为不同签名。

**绕过：** 脚本里所有 `printf` 统一只用 **1 个变参**（即格式串后只接一个值），多参数场景改用 `puts` 拼字符串。脚本第 [16] 节做了示范。

---

## 文件清单

| 文件 | 说明 |
|---|---|
| **all_features.toy** | **本 README 主要讲解对象**：18 节完整功能演示 |
| sin.toy / cos.toy | 单函数极简示例，演示 math 库 |
| array_basic.toy | 动态数组基础（创建 + 索引 + 长度） |
| array_iteration.toy | while 遍历动态数组求和 + `array_set` |
| array_resize.toy | 动态数组从空开始 push |
| matrix_mkl.toy | 调 Intel MKL 库做 2×2 矩阵乘法（需 `--features mkl`） |

> **注意：** 此前 `array_basic.toy / array_iteration.toy / array_resize.toy` 因 **Bug 1** 都跑不通；修复后应该都能正常编译运行。
