# 基于Cranelift的Toy语言编译器技术报告（版本 1.1）

作者：Trae AI Assistant  
日期：2026-02-04  
适用读者：熟悉 Rust 的导师（无编译原理背景）

## 目录
- 1. 原编译器架构深度解析
- 2. 我的修改工作完整说明（按 5 次提交）
- 3. 代码实现细节教学式解释
- 4. 图表索引（SVG）
- 5. 代码索引
- 6. 术语表
- 7. 参考资料

---

## 1. 原编译器架构深度解析

本章将对 Toy 编译器的核心架构进行系统性解剖，从组件交互、数据结构设计到 Rust 特有的实现细节进行深度展开。

### 1.1 核心编译组件逐级解剖

#### 1.1.1 词法与语法分析器 (Lexer & Parser)
*   **文件路径**: `src/frontend.rs`
*   **模块层级**: `crate::frontend`
*   **功能**: 将源代码字符串转换为抽象语法树 (AST)。本项目采用 `rust-peg` 库，通过 PEG (Parsing Expression Grammar) 宏将词法分析与语法分析合并处理，避免了独立的 Token 流生成阶段，支持无限向前搜索 (infinite lookahead)。

**公共 API 签名**:
```rust
// 生成的 parser 模块公共接口
pub fn function(input: &str) 
    -> Result<(String, Vec<(String, Type)>, (String, Type), Vec<Expr>), peg::error::ParseError<peg::str::LineCol>>
```

**关键实现片段 (PEG 宏定义)**:
```rust
peg::parser!(pub grammar parser() for str {
    // 词法规则：隐式处理空白符
    rule _() = quiet!{[' ' | '\t']*}

    // 语法规则：函数定义
    // 所有权注释：解析生成的 String 和 Vec<Expr> 所有权归属于返回的元组，
    // 在解析过程中，peg 负责从 input 切片中分配新的 String。
    pub rule function() -> (String, Vec<(String, Type)>, (String, Type), Vec<Expr>)
        = _ "fn" _ name:identifier() _ 
          "(" params:((_ i:identifier() _ ":" _ t:type_name() _ {(i, t)}) ** ",") ")" _
          "->" _ "(" ret:(_ i:identifier() _ ":" _ t:type_name() _ {(i, t)}) ")" _
          "{" _ "\n" stmts:statements() _ "}" _ "\n"
          { (name, params, ret, stmts) }
});
```

**性能与复杂度分析**:
*   **时间复杂度**: PEG 算法在最坏情况下为指数级，但通过 Packrat Parsing (记忆化) 可优化至 $O(N)$。本项目使用的 `rust-peg` 主要基于递归下降，未完全启用 Packrat，对于深度嵌套表达式可能存在回溯开销。
*   **空间复杂度**: $O(Depth)$，取决于递归深度。
*   **缓存友好度**: 低。AST 节点通过 `Box` 分散在堆上，遍历时易产生 Cache Miss。

**测试覆盖目标**:
*   单元测试覆盖率：100% (关键语法规则)。
*   Property-based 测试：生成随机深度的算术表达式以验证解析器鲁棒性。

#### 1.1.2 语义分析器 (Semantic Analyzer)
*   **文件路径**: `src/type_checker.rs`
*   **模块层级**: `crate::type_checker`
*   **功能**: 执行类型推导、函数签名解析与内置函数注册。目前主要在 IR 生成阶段即时调用 (`infer_type`)，而非独立的 Pass。

**公共 API 签名**:
```rust
// 纯函数设计，无副作用
// input: expr (借用 AST), get_var_type (闭包回调，用于查找符号表)
// output: Type (推导出的类型)
pub fn infer_type(expr: &Expr, get_var_type: &impl Fn(&str) -> Option<Type>) -> Type
```

**关键算法 (类型推导)**:
```rust
pub fn infer_type(expr: &Expr, get_var_type: &impl Fn(&str) -> Option<Type>) -> Type {
    match expr {
        // 基础情况：字面量直接返回携带类型
        Expr::Literal(_, ty) => ty.clone(),
        // 递归情况：二元运算，推导左操作数类型（假设左右同构或支持提升）
        Expr::Add(lhs, _) => infer_type(lhs, get_var_type),
        // 变量查找：调用回调函数查询作用域
        Expr::Identifier(name) => get_var_type(name).unwrap_or(Type::I64),
        // ...
    }
}
```
toolName: view_files
            
status: failed
          
            
filePath: d:\trae_code\cranelift-jit-demo\src\runtime.rs
          

        
            
toolName: view_folder
            
status: success
          
d:\trae_code\cranelift-jit-demo\src
          

        
            
          
整个 `sin`、`cos` 等函数的调用机制是一个完整的**从上层定义到底层实现**的闭环。它不是魔法，而是通过 4 个关键文件协同工作完成的。

简单来说，就是：**有人负责干活（实现），有人负责登记（注册），有人负责核对身份（签名），有人负责发号施令（编译调用）。**

以下是详细的实现链路：

### 1. 物理实现层：负责“干活”
这些函数的真正逻辑代码是用 Rust 写好，并暴露为 C 语言接口（ABI）供 JIT 调用的。
*   **文件**: [`src/runtime/math.rs`](file:///d:/trae_code/cranelift-jit-demo/src/runtime/math.rs)
*   **代码**:
    ```rust
    // src/runtime/math.rs
    #[unsafe(no_mangle)]
    pub extern "C" fn toy_sin(x: c_double) -> c_double {
        x.sin() // 调用 Rust 标准库的 f64::sin
    }
    ```
    这里定义了 `toy_sin`，它就像一个标准的 C 函数，JIT 编译出的机器码可以直接跳转到这里执行。

### 2. 符号注册层：负责“登记”
光有函数还不行，必须把这个函数的**名字（字符串）**和**内存地址（函数指针）**告诉 JIT 编译器，建立映射关系。
*   **文件**: [`src/runtime/registry.rs`](file:///d:/trae_code/cranelift-jit-demo/src/runtime/registry.rs)
*   **代码**:
    ```rust
    // src/runtime/registry.rs
    pub fn register_builtins(builder: &mut JITBuilder) {
        // ...
        // 告诉 JIT: 当你看到 "sin" 这个符号时，它的地址是 math::toy_sin
        builder.symbol("sin", math::toy_sin as *const u8);
        builder.symbol("cos", math::toy_cos as *const u8);
        // ...
    }
    ```
    这一步通常在编译器启动初始化（`JIT::default()`）时执行。

### 3. 类型元数据层：负责“核对身份”
编译器在生成调用指令前，需要知道这个函数接受几个参数，参数是什么类型，返回值是什么类型，以便生成正确的寄存器传递代码。
*   **文件**: [`src/type_checker.rs`](file:///d:/trae_code/cranelift-jit-demo/src/type_checker.rs)
*   **代码**:
    ```rust
    // src/type_checker.rs
    self.functions.insert("sin".to_string(), FunctionSignature {
        params: vec![Type::F64], // 参数是 F64
        ret: Type::F64,          // 返回值是 F64
        is_external: true,       // 标记为外部函数
    });
    ```

### 4. 指令生成层：负责“发号施令”
最后，在编译阶段，当编译器遇到用户写的 `sin(3.14)` 时，它会查表并生成调用指令。
*   **文件**: [`src/jit.rs`](file:///d:/trae_code/cranelift-jit-demo/src/jit.rs)
*   **代码**: `translate_call` 方法 (约 L531)
    ```rust
    // src/jit.rs
    fn translate_call(&mut self, name: String, args: Vec<Expr>) -> Value {
        // 1. 查类型表 (TypeChecker)，确定参数和返回值类型
        let signature = self.type_checker.resolve_func(&name);
        
        // ... 构建 Cranelift 的 Signature ...

        // 2. 声明这是一个导入的函数 (Linkage::Import)
        // Cranelift 会去之前的注册表里找 "sin" 对应的地址
        let callee = self.module.declare_function(&name, Linkage::Import, &sig)...;
        
        // 3. 生成 call 指令
        let call = self.builder.ins().call(local_callee, &arg_values);
        // ...
    }
    ```

### 总结
当你写下 `sin(x)` 时：
1.  **`src/jit.rs`** 看到调用，问 **`src/type_checker.rs`**：“`sin` 长啥样？”
2.  拿到签名后，**`src/jit.rs`** 生成一条“去调用外部函数 `sin`”的机器指令。
3.  程序运行时，JIT 系统根据 **`src/runtime/registry.rs`** 的记录，将这个调用导向 **`src/runtime/math.rs`** 里的 `toy_sin` 代码地址。
4.  最终执行了 Rust 标准库的 `f64::sin`。



#### 1.1.3 中间代码生成器 (IR Generator)
*   **文件路径**: `src/jit.rs`
*   **模块层级**: `crate::jit`
*   **功能**: 将 AST 翻译为 Cranelift IR (CLIF)。负责 SSA (Static Single Assignment) 构建、变量声明 (`declare_variables`)、基本块 (`BasicBlock`) 管理与控制流图 (CFG) 生成。

**关键算法 (AST 遍历至 IR 指令)**:
```rust
// 访问者模式变体
fn translate_expr(&mut self, expr: Expr) -> Value {
    match expr {
        Expr::Add(lhs, rhs) => {
            // 递归生成操作数指令，获取 SSA Value
            let l_val = self.translate_expr(*lhs); 
            let r_val = self.translate_expr(*rhs);
            // 插入类型提升指令 (Promote)
            let (l, r) = self.promote_operands(l_val, r_val);
            // 插入加法指令
            if self.is_float(l) {
                self.builder.ins().fadd(l, r)
            } else {
                self.builder.ins().iadd(l, r)
            }
        }
        // ...
    }
}
```

### 1.2 关键数据结构的 Rust 定义与意图阐释

#### 1.2.1 抽象语法树 (AST)
```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    // 基础字面量：使用 String 存储原始内容，避免过早精度丢失
    // 内存布局：String (24字节) + Type (枚举大小)
    Literal(String, Type), 
    
    // 递归结构：使用 Box<T> 解决递归类型大小无限问题
    // 意图：Box 提供堆分配所有权，适合树状结构。
    // 替代方案：在高性能场景下，应使用 Arena (如 typed-arena) 配合 &'a Expr，
    // 以实现连续内存分配和零拷贝遍历。
    Add(Box<Expr>, Box<Expr>),
    
    // 控制流：拥有子代码块的所有权
    IfElse(Box<Expr>, Vec<Expr>, Vec<Expr>),
    
    // ...
}
```

*   **字符串选型分析**: 目前使用 standard `String`。
    *   **优点**: 拥有所有权，生命周期管理简单。
    *   **缺点**: 大量小字符串导致内存碎片和分配开销。
    *   **优化建议**: 生产环境应使用 `SmolStr` (内联小字符串优化) 或 `Interner` (字符串驻留池) 将标识符映射为 `u32` 符号 ID。

#### 1.2.2 类型系统 (Type System)
```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    I64, F64,              // 基础标量
    Complex128,            // 复合标量
    Array(Box<Type>, usize), // 复合类型，Box<Type> 指向元素类型
    DynamicArray(Box<Type>), 
}
```

### 1.3 编译流程全阶段数据流转与可视化

```mermaid
graph TD
    Source[源码输入 String] -->|字符流| Parser[PEG Parser]
    Parser -->|Result<AST>| AST[AST (Expr Tree)]
    
    subgraph Frontend
    AST -->|Borrow| TypeChecker[Type Checker]
    TypeChecker -.->|Inferred Type| AST
    end
    
    AST -->|Move| IRGen[IR Generator]
    IRGen -->|Builder Calls| CLIF[Cranelift IR (SSA)]
    
    subgraph Backend
    CLIF -->|Optimize| Optimizer[Cranelift Optimizer]
    Optimizer -->|MachInst| Codegen[Machine Code Gen]
    end
    
    Codegen -->|mmap| Exec[JIT Memory (r-x)]
```

**错误恢复策略**:
*   **Parser**: 目前采用 Panic Mode (即遇到错误直接返回 `Err`)。改进方案是引入 `Recovery` 规则，跳过错误 Token 直至同步点 (如 `;` 或 `}`)，从而收集多个错误。
*   **Semantic**: 类型不匹配时记录 `Diagnostic` 并继续推导 (使用 `ErrorType` 占位)，防止单次错误中断整个流程。

### 1.4 Rust 特有实现细节深度剖析

#### 1.4.1 所有权模型与 AST 遍历
在 `translate_expr` 中，我们采用了 **消费式遍历 (Consuming Traversal)**：
```rust
// fn translate_expr(&mut self, expr: Expr) -> Value
// expr 被 move 进函数，随即被解构 (match)
// 子节点 *lhs (Box<Expr>) 被解引用并移动到递归调用中
Expr::Add(lhs, rhs) => { ... self.translate_expr(*lhs) ... }
```
*   **优势**: 避免了 `.clone()`，AST 节点在翻译完成后立即释放，降低峰值内存。
*   **对比**: 若需多次遍历 (如先优化 AST 再生成 IR)，则需改用引用遍历 `&Expr` 或实现 `Visitor` trait。

#### 1.4.2 模式匹配与宏应用
为了减少重复的 IR 生成代码，推荐使用宏来处理二元运算：

```rust
macro_rules! match_binop {
    ($self:ident, $lhs:expr, $rhs:expr, $iop:ident, $fop:ident) => {{
        let l = $self.translate_expr(*$lhs);
        let r = $self.translate_expr(*$rhs);
        let (l, r) = $self.promote_operands(l, r);
        if $self.is_float(l) { $self.builder.ins().$fop(l, r) } 
        else { $self.builder.ins().$iop(l, r) }
    }}
}

// 调用示例
Expr::Add(lhs, rhs) => match_binop!(self, lhs, rhs, iadd, fadd),
```

### 1.5 可扩展补充内容

*   **增量编译接口**: 引入 `DependencyGraph`，基于函数级 Hash (`Fingerprint`) 计算脏集。
*   **Fuzz 与 Miri 兼容性**: 
    *   `jit.rs` 中涉及 `unsafe` 的裸指针操作 (如 `GlobalDataAddr`) 需通过 Miri 验证。
    *   推荐在 CI 中添加 `MIRIFLAGS="-Zmiri-disable-isolation"` 运行测试。

---

## 2. 我的修改工作完整说明（按 5 次提交）

本节分别对 5 次主要提交进行“差异统计、逐文件行级 diff、动机与 Rust 特性说明、分类归档（算法/架构/功能/修复）”。

### 2.1 dbe85c2 Update: Add float and custom string features

**Git 统计**
- 6 files changed, 484 insertions(+), 250 deletions(-)
- 受影响文件：README.md, src/bin/toy.rs, src/frontend.rs, src/jit.rs, .trae/documents/Extend Cranelift JIT Demo Type System.md, check.log

**关键改动（逐文件摘要与行级 diff 片段）**

- src/frontend.rs：为 AST 与语法加入类型系统与显式 cast

```diff
1 - pub enum Expr { Literal(String), Identifier(String), ... }
2 + pub enum Expr { Literal(String, Type), Cast(Box<Expr>, Type), ... }
3
4 - pub rule function() -> (String, Vec<String>, String, Vec<Expr>)
5 + pub rule function() -> (String, Vec<(String, Type)>, (String, Type), Vec<Expr>)
6
7 + rule type_name() -> Type = "i8" | "i16" | "i32" | "i64" | "i128" | "f32" | "f64"
```

- src/bin/toy.rs：测试用例改为显式类型签名，新增浮点与混合类型案例

```diff
1 - fn foo(a, b) -> (c) { ... }
2 + fn foo(a: i64, b: i64) -> (c: i64) { ... }
3
4 + const FLOAT_ADD_CODE: &str = r#"
5 +     fn float_add(a: f64, b: f64) -> (c: f64) { c = a + b } "#;
6 + const MIXED_ADD_CODE: &str = r#"
7 +     fn mixed_add(a: i32, b: f64) -> (c: f64) { c = (a as f64) + b } "#;
```

- src/jit.rs：引入类型提升与显式 cast 的 IR 生成，使用 select 替代移除的 bint

```diff
1 + fn translate_cast(&mut self, val: Value, target_ty: types::Type) -> Value { ... }
2 + fn promote_operands(&mut self, lhs: Value, rhs: Value) -> (Value, Value) { ... }
3 + let one = iconst(I64, 1); let zero = iconst(I64, 0); builder.ins().select(bool_res, one, zero);
```

**逐行动机与特性说明**
- 为 Expr 引入 Type 与 Cast：使语言支持多数值类型与显式转换；借助 Rust 枚举表达语法树分支；Box<Expr> 解决递归类型大小
- JIT 层加入 promote/cast：借助 Cranelift 的 sextend/fpromote/fcvt 指令；select(cond, 1, 0) 适配 Cranelift 0.125 移除 bint
- 测试用例显式签名：利用类型系统提高可读性与安全性；mem::transmute 以 C ABI 调用已 JIT 编译函数

**分类归档**
- 算法改进：类型提升规则（int/float 宽化），复杂度 O(1)
- 架构重构：前端签名从无类型改为带类型，type_checker 引入为后续扩展铺路
- 新功能实现：浮点计算、显式 as 转换、字符串字面量输出
- bug 修复：bint 移除后的布尔到整数转换替换为 select，避免验证错误

### 2.2 cd90a76 feat: 扩展前端语法并支持复杂类型和数组

**Git 统计（主要受影响）**
- Cargo.toml 新增 libc
- src/frontend.rs 扩展：字符串、复数、数组、索引等
- src/jit.rs 扩展：字符串数据定义、复数与数组 IR 生成、索引与越界 trap
- src/bin/toy.rs 扩展：更多端到端测试样例

**关键改动（逐文件摘录）**
- frontend.rs：加入 String/Complex/Array 语法与类型

```diff
1 + StringLiteral(String), ComplexLiteral(f64, f64, Type), ArrayLiteral(Vec<Expr>, Type)
2 + Index(Box<Expr>, Box<Expr>)
3 + type_name(): string | complex64 | complex128 | [T; N]
4 + array_literal(): "[" elems ... "]"
5 + string_literal with escapes, complex_literal with "i"
```

- jit.rs：字符串常量终止符、复数打包、数组分配与索引越界检测

```rust
1 // 字符串
2 let mut bytes = s.as_bytes().to_vec(); bytes.push(0); define data; symbol_value(ptr)
3 // 复数
4 // Complex64: pack two f32 into I64；Complex128: stack 16B，读写 I128
5 // 数组与索引
6 trapnz(out_of_bounds, TrapCode::unwrap_user(1)); load(elem_ty, addr, 0)
```

**动机与特性**
- 丰富语言前端的表达力（字符串/复数/数组），匹配实际计算任务需求
- 索引越界 trap 保证内存安全；字符串 null-terminate 兼容 libc
- 复数通过位打包/栈槽实现零拷贝与类型规整

**分类**
- 算法：复数运算公式实现（Add/Sub/Mul/Div），每步 O(1)
- 架构：前端/后端协同扩展类型系统；IR 显式类型与数据布局
- 功能：字符串 I/O、数组字面量、索引与越界检测
- 修复：then/else 类型统一与 merge block 参数一致，避免验证问题

### 2.3 e60f380 feat: 添加运行时函数、类型检查和基准测试

**Git 统计**
- 新增：src/runtime/extern_functions.rs、src/type_checker.rs、benches/jit_bench.rs、tests/*
- 修改：src/jit.rs、src/lib.rs、Cargo.toml（nalgebra、criterion）

**关键改动**
- runtime/extern_functions.rs：数学函数、随机数、sum_array/print_matrix_2x2

```rust
1 #[unsafe(no_mangle)] pub extern "C" fn toy_sin(x: c_double) -> c_double { x.sin() }
2 #[unsafe(no_mangle)] pub extern "C" fn toy_rand() -> i64 { rng.random::<i32>() as i64 }
3 #[unsafe(no_mangle)] pub unsafe extern "C" fn toy_sum_array(ptr: *const f64, len: usize) -> f64 { ... }
```

- type_checker.rs：函数签名注册与 infer_type

```rust
1 #[derive(Clone, Debug)] pub struct FunctionSignature { params: Vec<Type>, ret: Type, is_external: bool }
2 impl TypeChecker { fn register_builtins(&mut self) { ... } }
3 pub fn infer_type(expr: &Expr, get_var_type: &impl Fn(&str)->Option<Type>) -> Type { ... }
```

- benches 与 tests：端到端验证

**动机与特性**
- 将外部符号集中管理，提升可扩展性；类型检查器为 IR 生成提供依据
- 基准测试与集成测试提高工程可验证性；Rust 的 #[unsafe(no_mangle)] 与 C ABI 保证 JIT 可调用

**分类**
- 算法：类型推导规则（递归匹配），复杂度 O(AST)
- 架构：引入类型检查器与运行时模块，解耦 JIT
- 功能：数学库/数组求和/打印矩阵
- 修复：调用约定与返回值一致性（printf/puts 返回 I32）

### 2.4 538bef0 feat: 集成 Intel MKL 并替换 nalgebra 依赖

**Git 统计（摘）**
- Cargo.toml：移除 nalgebra，新增 intel-mkl-src、build.rs
- runtime/extern_functions.rs：移除 nalgebra 相关，新增 cblas_dgemm 与 toy_mkl_dgemm 包装
- src/jit.rs：JITBuilder 注册 MKL 符号
- tests/integration_test.rs：新增 MKL DGEMM 用例

**行级变更片段**

```diff
1 - use nalgebra::{SMatrix};
2 + // Intel MKL FFI
3 + unsafe extern "C" { pub fn cblas_dgemm(...); }
4 + #[unsafe(no_mangle)]
5 + pub unsafe extern "C" fn toy_mkl_dgemm(m: i64, n: i64, k: i64, alpha: f64, a_ptr: *const f64, _a_len: usize, beta: f64, b_ptr: *const f64, _b_len: usize, c_ptr: *mut f64, _c_len: usize) { cblas_dgemm(...); }
```

**动机与特性**
- 性能优先：DGEMM 由 MKL 高度优化实现；JIT 通过外部符号直连
- Rust FFI：extern "C" 与 no_mangle，结合 feature mkl 控制编译
- 类型检查器将数组参数识别为外部调用需展开（ptr, len），保证 ABI 一致

**分类**
- 算法：DGEMM 调用属于库级优化
- 架构：外部库替换与 FFI 注册，解耦前端与内部数学实现
- 功能：新增 MKL 用例与指南
- 修复：删除旧 nalgebra 测试，避免重复与冲突

### 2.5 79ab48f feat(memory): 基于 RAII 的动态数组实现与集成

**Git 统计（摘）**
- 新增 memory_bench/raii_demo/*（DynamicArray<T> 实现、bench、examples、CI）
- Cargo.toml：新增路径依赖 raii_demo
- src/runtime/array.rs：桥接到 DynamicArray<T> 的 C ABI

**关键实现（摘录）**

```rust
1 pub struct DynamicArray<T> { ptr: NonNull<T>, cap: usize, len: usize, _marker: PhantomData<T> }
2 impl<T> DynamicArray<T> {
3     pub fn new() -> Self { ... } pub fn push(&mut self, elem: T) { ... } pub fn pop(&mut self) -> Option<T> { ... }
4     fn grow(&mut self) { ... } fn do_realloc(&mut self, new_cap: usize) -> Result<(), AllocError> { ... }
5 }
```

```rust
1 // src/runtime/array.rs
2 #[unsafe(no_mangle)] pub extern "C" fn dynamic_array_new_i64() -> *mut DynamicArray<i64> { ... }
3 #[unsafe(no_mangle)] pub extern "C" fn dynamic_array_push_i64(arr_ptr: *mut DynamicArray<i64>, elem: i64) -> i64 { ... }
4 #[unsafe(no_mangle)] pub extern "C" fn dynamic_array_drop_i64(arr_ptr: *mut DynamicArray<i64>) -> i64 { ... }
```

**动机与特性**
- 内存安全与零成本抽象：RAII 自动析构，NonNull 管理裸指针，Drop 保证泄漏安全
- JIT 与运行时桥接：Toy 语言的 DynamicArray 字面量通过运行时 API 创建与操作
- CI 集成（Miri/Valgrind）：加强 Unsafe 代码的行为验证

**分类**
- 算法：扩容策略摊销 O(1)，insert/remove O(N)
- 架构：引入独立库 raii_demo 并通过运行时注册与 JIT 对接
- 功能：动态数组字面量、push/pop/len/cap/get_ptr/set/drop
- 修复：显式释放动态数组，避免跨函数泄漏

---

## 3. 代码实现细节教学式解释

**函数签名与实现（重点）**

- AST → IR 翻译入口：translate_expr

```rust
1 fn translate_expr(&mut self, expr: Expr) -> Value {
2     match expr {
3         Expr::Add(lhs, rhs) => {
4             let ty = type_checker::infer_type(&lhs, &|n| self.variables.get(n).map(|(_, t)| t.clone()));
5             if is_complex(&ty) { self.translate_complex_binop(*lhs, *rhs, BinOp::Add) }
6             else { self.translate_binary_op(*lhs, *rhs, |b, l, r| { let ty = b.func.dfg.value_type(l); if ty.is_float(){ b.ins().fadd(l,r)} else { b.ins().iadd(l,r)} }) }
7         }
8         Expr::Cast(expr, target_ty) => { let v = self.translate_expr(*expr); self.translate_cast(v, to_cranelift_type(&target_ty)) }
9         Expr::StringLiteral(s) => self.translate_string_literal(s),
10         Expr::ArrayLiteral(elems, ty) => self.translate_array_literal(elems, ty),
11         Expr::Index(base, idx) => self.translate_index(*base, *idx),
12         _ => { /* 其他分支同理 */ unimplemented!() }
13     }
14 }
```

**参数与返回值说明**
- 采用 SSA 值 Value 表示 IR 里的临时变量
- 二元操作根据 Cranelift 类型分派至 iadd/fadd 等指令
- cast 使用 sextend/ireduce/fpromote/fdemote/fcvt 系列
- 字符串通过 DataDescription 定义只读段并返回符号地址

**关键库函数**
- std::collections::HashMap：变量名到 (Variable, Type) 的映射；不特别设置容量，按语句扫描增量扩张
- libc：printf/puts 函数符号用于 I/O 输出
- peg：precedence! 宏定义运算优先级；identifier/literal 等规则捕获
- criterion：基准测试定义与运行

**算法执行示例**
- AST → 符号表填充：declare_variables_in_stmt 扫描赋值语句并声明变量
- 索引流程：计算 idx 的 I64，做越界检查，addr = base + idx * elem_size，然后 load
- 复数运算：Complex64 将两个 f32 打包在 I64 的低/高 32 位；运算后再打包

**Rust 实现技巧**
- 访问者模式：match Expr 分派访问；各子翻译函数清晰独立
- mem::replace：用于一些需要值移动的场景（此项目中主要使用栈槽复合类型）
- Cow：当前未广泛使用，建议未来在字符串常量复用场景引入以减少分配

---

## 4. 图表索引（SVG）
- 图 1：编译流程图（第 1 章 SVG）
- 图 2：模块依赖关系图（后续可补充独立 SVG 文件）

---

## 5. 代码索引
- [Cargo.toml](file:///d:/trae_code/cranelift-jit-demo/Cargo.toml)
- [frontend.rs](file:///d:/trae_code/cranelift-jit-demo/src/frontend.rs)
- [jit.rs](file:///d:/trae_code/cranelift-jit-demo/src/jit.rs)
- [type_checker.rs](file:///d:/trae_code/cranelift-jit-demo/src/type_checker.rs)
- [runtime/registry.rs](file:///d:/trae_code/cranelift-jit-demo/src/runtime/registry.rs)
- [runtime/array.rs](file:///d:/trae_code/cranelift-jit-demo/src/runtime/array.rs)
- [runtime/math.rs](file:///d:/trae_code/cranelift-jit-demo/src/runtime/math.rs)
- [runtime/io.rs](file:///d:/trae_code/cranelift-jit-demo/src/runtime/io.rs)
- [runtime/mkl.rs](file:///d:/trae_code/cranelift-jit-demo/src/runtime/mkl.rs)
- [bin/toy.rs](file:///d:/trae_code/cranelift-jit-demo/src/bin/toy.rs)
- [cli/mod.rs](file:///d:/trae_code/cranelift-jit-demo/src/cli/mod.rs)
- [tests/integration_test.rs](file:///d:/trae_code/cranelift-jit-demo/tests/integration_test.rs)

---

## 6. 术语表
- AST：抽象语法树
- IR：中间表示，Cranelift IR（CLIF）
- SSA：静态单赋值
- ABI：应用二进制接口（extern "C"）
- RAII：资源获取即初始化
- DGEMM：双精度矩阵乘法

---

## 7. 参考资料
- Rust 官方文档：https://doc.rust-lang.org/book/
- Cranelift 文档：https://docs.rs/cranelift/latest/cranelift/
- rust-peg 项目：https://github.com/kevinmehall/rust-peg
- SSA 介绍：https://en.wikipedia.org/wiki/Static_single_assignment_form
- Intel MKL 文档：https://www.intel.com/content/www/us/en/developer/tools/oneapi/onemkl.html
- anyhow 文档：https://docs.rs/anyhow
- clap 文档：https://docs.rs/clap
- criterion 文档：https://docs.rs/criterion
