# RAII 风格生命周期管理 — 合并实施方案（v2）

> **状态**：✅ 已实施（2026-06-09，4 Phase 完成）
> **实际代码量**：净增 ~190 行（计划估算 400–500 行，简化后远低于预期）
> **v2 变更**：修复 PEG `\n` 限制、补充 type_checker、明确 Block 语义、统一所有权追踪、修复 while 循环泄漏

---

## 〇、目标

将 DynamicArray 的释放从"统一推迟到函数末尾"改为"尽可能早地释放"，接近 RAII 的语义。三条规则：

1. **块作用域**：`{ }` 块内定义的 DynamicArray，生命周期不超过该块。块退出时兜底释放。
2. **循环迭代释放**：`while` 循环体内定义的 DynamicArray，每次迭代结束时释放（修复迭代间泄漏）。
3. **活跃度分析**（后续迭代）：在所在作用域内，找到变量的"最后一次使用"位置，在那之后立即释放。

---

## 一、AST 与语法层

### 1.1 新增 AST 节点

**文件**：[`src/frontend.rs`](src/frontend.rs) — `Expr` 枚举

```rust
pub enum Expr {
    // ... 现有 25 个变体保持不变 ...
    /// 块作用域：{ stmt1; stmt2; ... }
    /// 作为语句使用（不产生有意义的值，返回 0:i64）。
    /// 块内定义的 DynamicArray 在块退出时自动 drop。
    Block(Vec<Expr>),
}
```

位置：在现有变体列表末尾追加。

**语义定位：语句级 Block**。Block 放在 PEG 的 `statement()` 规则中（与 `if_else`、`while_loop` 同级），不放在 `expression()` 中。这意味着 Block 不能嵌套在表达式中（如 `x = { ... }`），只能作为独立语句使用。这简化了 JIT 翻译——`translate_block` 无需返回值。

> **为什么不是表达式级 Block？** Toy 语言不需要块表达式。将 Block 限制为语句级可以避免"块的最后表达式值"的语义复杂性，也无需在 `infer_type` 中处理块类型推导。如果未来需要表达式级 Block，可以无痛升级。

### 1.2 新增 PEG 语法规则

**文件**：[`src/frontend.rs`](src/frontend.rs) — `peg::parser!` 宏内

```rust
// statement() 规则中新增 block_stmt：
rule statement() -> Expr
    = if_else()
    / while_loop()
    / block_stmt()            // ← 新增
    / "drop" _ "(" _ i:identifier() _ ")" { Expr::Drop(i) }
    / assignment()
    / binary_op()

// 块语句：{ stmts }
// 无歧义：PEG 是有序选择，if/while 以关键字开头，不会匹配独立的 {
rule block_stmt() -> Expr
    = "{" _ body:statements() _ "}" _ { Expr::Block(body) }
```

**消歧说明**：PEG 使用有序选择（ordered choice）。`statement()` 中先尝试 `if_else()`（前缀为 `if`），再尝试 `while_loop()`（前缀为 `while`），两者都不可能匹配独立的 `{`。因此 `{` 唯一匹配 `block_stmt()`，**不需要**在 `{` 后强制换行或任何其他消歧手段。`{ a = array [1]; }` 这样的单行写法合法。

### 1.3 新增 type_checker 处理

**文件**：[`src/type_checker.rs`](src/type_checker.rs) — `infer_type` 函数

```rust
Expr::Block(stmts) => {
    // 块本身不产生有意义的值，返回 I64 作为占位
    // Block 是语句级构造，不会出现在需要类型推导的表达式中
    FrontendType::I64
}
```

这是防御性分支——Block 作为语句不会出现在表达式上下文中，`infer_type` 不太可能被调用到。但为完整性和未来可能的表达式级升级，保留此分支。

### 1.4 同步修改的上下游

| 文件 | 需要修改的代码 | 原因 |
|---|---|---|
| [`src/frontend.rs`](src/frontend.rs) — `Expr` | 添加 `Block` 变体 | AST 定义 |
| [`src/frontend.rs`](src/frontend.rs) — PEG | `statement()` 添加 `block_stmt()` 规则 | 解析 |
| [`src/type_checker.rs`](src/type_checker.rs) | `infer_type` 添加 `Expr::Block` 分支 | 防御性完整 |
| [`src/optimizer.rs`](src/optimizer.rs) | `fold_constants` 添加 `Expr::Block(stmts)` 分支 | 递归优化块内语句 |
| [`src/jit.rs`](src/jit.rs) — `declare_variables_in_stmt` | 添加 `Expr::Block` 分支 | 扫描块内隐式变量 |
| [`src/jit.rs`](src/jit.rs) — `translate_expr` | 添加 `Expr::Block` 分支 | JIT 翻译 |
| [`src/ownership.rs`](src/ownership.rs) | 全面改造，输出 `ScopeAnalysis` | 按作用域递归分析，统一所有权追踪 |

---

## 二、所有权检查器改造 —— 输出 ScopeAnalysis

### 2.1 核心思路：统一所有权追踪

**当前问题**：`ownership.rs` 和 `jit.rs` 各自维护一套 DynamicArray 追踪逻辑。ownership 做扁平扫描，jit 做 flat `dynamic_arrays` 列表。两套系统独立运作，信息不互通——代码中已有 FIXME 注释指出此问题。

**v2 方案**：ownership checker 改造为输出一个 **`ScopeAnalysis`** 结构体，JIT 编译器直接消费该结构体，不再独立追踪作用域。从"两套系统各自维护"变为"一套分析，两处消费"。

### 2.2 共享数据结构

新增文件或在 `ownership.rs` 顶部定义：

```rust
/// 由 OwnershipChecker 输出的作用域分析结果。
/// JIT 编译器消费此结构，无需独立追踪作用域。
#[derive(Debug, Clone)]
pub struct ScopeAnalysis {
    /// scope_depth -> 该作用域内定义的 DynamicArray 变量名列表
    /// scope_depth=0 为函数体顶层
    pub scope_vars: HashMap<usize, Vec<String>>,
    /// 循环体的作用域深度列表。
    /// 这些作用域在每次迭代结束时需要释放（而非仅在退出时释放一次）。
    pub loop_scopes: HashSet<usize>,
    /// 变量名 -> 显式 drop 发生所在的作用域深度
    /// JIT 在对应深度的 auto-drop 时跳过这些变量
    pub explicit_drops: HashMap<String, usize>,
}
```

### 2.3 OwnershipChecker 新数据结构

```rust
pub struct OwnershipChecker {
    /// 当前作用域中已登记的 DynamicArray 变量
    /// 键 = 变量名，值 = (disposition, 定义所在的作用域深度)
    arrays: HashMap<String, (ArrayInfo, usize)>,
    /// 累积的错误列表
    errors: Vec<OwnershipError>,
    /// 当前作用域深度。0 = 函数体顶层，每进入一层 Block/While body +1。
    scope_depth: usize,
    /// 每个作用域结束时需要检查的变量名集合
    /// 键 = 作用域深度，值 = 该作用域内定义的所有 DynamicArray 变量名
    scope_vars: HashMap<usize, Vec<String>>,
    /// 循环体的作用域深度（WhileLoop body 的深度）
    loop_scopes: HashSet<usize>,
    /// 变量名 -> 显式 drop 所在的作用域深度
    explicit_drops: HashMap<String, usize>,
}
```

### 2.4 核心算法

```rust
pub fn analyze_function(
    &mut self,
    _params: &[(String, Type)],
    stmts: &[Expr],
    return_var: &str,
) -> (ScopeAnalysis, Vec<OwnershipError>) {
    self.scope_depth = 0;
    self.analyze_stmts(stmts, return_var);
    // 函数体顶层结束时：
    //   - scope_depth=0 中仍为 Owned → LeakedArray
    //   - Returned/Dropped/Passed 的变量从 arrays 中移除
    self.close_scope(0);

    let analysis = ScopeAnalysis {
        scope_vars: self.scope_vars.clone(),
        loop_scopes: self.loop_scopes.clone(),
        explicit_drops: self.explicit_drops.clone(),
    };
    (analysis, self.errors.clone())
}

fn analyze_stmts(&mut self, stmts: &[Expr], return_var: &str) {
    for stmt in stmts {
        self.analyze_expr(stmt, return_var);
    }
}

fn analyze_expr(&mut self, expr: &Expr, return_var: &str) {
    match expr {
        // --- 赋值 ---
        Expr::Assign(name, value) => {
            let produces_array = Self::expr_produces_dynarray(value);
            if produces_array {
                // 覆盖检测：如果变量已存在且为 Owned → 旧数组泄漏
                if let Some((old_info, _)) = self.arrays.get(name)
                    && old_info.disposition == ArrayDisposition::Owned
                {
                    self.errors.push(OwnershipError::LeakedArray {
                        name: format!("{} (previous value overwritten)", name),
                    });
                }
                // 登记到当前作用域
                self.arrays.insert(
                    name.clone(),
                    (ArrayInfo {
                        disposition: ArrayDisposition::Owned,
                        name: name.clone(),
                    }, self.scope_depth),
                );
                self.scope_vars.entry(self.scope_depth)
                    .or_default()
                    .push(name.clone());
            }
            // 赋值给返回值：标记源变量为 Returned
            if name == return_var {
                if let Expr::Identifier(src) = value {
                    if let Some((info, _)) = self.arrays.get_mut(src) {
                        info.disposition = ArrayDisposition::Returned;
                    }
                }
            }
        }

        // --- 显式 drop ---
        Expr::Drop(name) => {
            self.mark_dropped(name);
            self.explicit_drops.insert(name.clone(), self.scope_depth);
        }

        // --- 函数调用：实参为 Owned → Passed ---
        Expr::Call(_, args) => {
            for arg in args {
                if let Expr::Identifier(name) = arg {
                    if let Some((info, _)) = self.arrays.get_mut(name)
                        && info.disposition == ArrayDisposition::Owned
                    {
                        info.disposition = ArrayDisposition::Passed;
                    }
                }
            }
        }

        // --- Block 节点（新增） ---
        Expr::Block(body) => {
            self.scope_depth += 1;
            self.scope_vars.insert(self.scope_depth, Vec::new());
            self.analyze_stmts(body, return_var);
            self.close_scope(self.scope_depth);
            self.scope_depth -= 1;
        }

        // --- While 循环：体作为循环作用域 ---
        // 注意：WhileLoop 的 body 字段类型为 Vec<Expr>（语句列表）
        Expr::WhileLoop(_cond, body) => {
            self.scope_depth += 1;
            self.loop_scopes.insert(self.scope_depth);
            self.scope_vars.insert(self.scope_depth, Vec::new());
            self.analyze_stmts(body, return_var);
            self.close_scope(self.scope_depth);
            self.scope_depth -= 1;
        }

        // --- If/Else：保守策略，登记到当前作用域 ---
        Expr::IfElse(_cond, then_body, else_body) => {
            self.analyze_stmts(then_body, return_var);
            if let Some(else_stmts) = else_body {
                self.analyze_stmts(else_stmts, return_var);
            }
        }

        // --- 索引：检测 UseAfterDrop ---
        Expr::Index(base, _idx) => {
            if let Expr::Identifier(name) = base.as_ref() {
                if let Some((info, _)) = self.arrays.get(name)
                    && info.disposition == ArrayDisposition::Dropped
                {
                    self.errors.push(OwnershipError::UseAfterDrop {
                        name: name.clone(),
                    });
                }
            }
        }

        _ => {}
    }
}
```

### 2.5 `close_scope`：作用域退出时的检查

```rust
fn close_scope(&mut self, depth: usize) {
    if let Some(vars) = self.scope_vars.get(&depth) {
        for name in vars {
            if let Some((info, _)) = self.arrays.get(name) {
                match info.disposition {
                    // 作用域结束时仍为 Owned → 泄漏！
                    ArrayDisposition::Owned => {
                        self.errors.push(OwnershipError::LeakedArray {
                            name: name.clone(),
                        });
                    }
                    // Returned/Dropped/Passed → 合法，无需处理
                    _ => {}
                }
            }
            self.arrays.remove(name);
        }
    }
}
```

### 2.6 `expr_produces_dynarray`：判断表达式是否产生 DynamicArray

新增辅助方法，供 ownership checker 判断赋值 RHS 是否创建了 DynamicArray：

```rust
fn expr_produces_dynarray(expr: &Expr) -> bool {
    match expr {
        Expr::DynamicArrayLiteral(_) => true,
        Expr::Call(fname, _) => {
            fname == "array_new_i64"
                || fname == "array_new_f64"
                || fname == "array_new_complex128"
        }
        // 函数调用返回值、索引赋值等也可能产生 DynamicArray
        // 保守起见仅匹配明确的构造形式
        _ => false,
    }
}
```

### 2.7 分支内的作用域处理

`IfElse` 保守策略（与现有行为一致）：分支内定义的数组登记到**当前作用域**（父作用域），不做跨分支合并分析。如果只在 if 分支内 drop 而 else 分支没处理，则 else 路径上的泄漏会在运行时暴露。

`WhileLoop`（v2 新增）：循环体作为独立作用域（`loop_scope`）。每次迭代结束时释放循环体内定义的数组（见 JIT 部分 3.7），避免迭代间泄漏。

---

## 三、JIT 编译器改造 —— 消费 ScopeAnalysis

### 3.1 FunctionTranslator 结构变更

```rust
struct FunctionTranslator<'a> {
    builder: FunctionBuilder<'a>,
    variables: HashMap<String, (Variable, FrontendType)>,
    module: &'a mut JITModule,
    current_func_name: String,
    current_func_ret_type: types::Type,
    string_counter: usize,
    type_checker: &'a TypeChecker,

    /// 作用域分析结果（由 ownership checker 预计算）
    scope_analysis: ScopeAnalysis,
    /// 当前作用域深度。0 = 函数体顶层，每进入一层 Block +1。
    scope_depth: usize,
    /// 已显式 drop 的 Cranelift Variable 集合（翻译时填充）
    explicitly_dropped: Vec<Variable>,
}
```

**关键变化**：`scoped_arrays: Vec<Vec<...>>` 栈被替换为 `scope_analysis: ScopeAnalysis` + `scope_depth: usize` 计数器。JIT 不再独立追踪每个数组属于哪个作用域——在作用域退出时，直接查询 `scope_analysis.scope_vars[&scope_depth]`。

### 3.2 `emit_drop_call` 辅助方法

```rust
/// 发射一条 `call array_drop_xxx(val)` IR 指令
fn emit_drop_call(&mut self, drop_func_name: &str, val: Value) {
    let mut drop_sig = self.module.make_signature();
    drop_sig.params.push(AbiParam::new(types::I64));
    drop_sig.returns.push(AbiParam::new(types::I64));
    let drop_callee = self.module
        .declare_function(drop_func_name, Linkage::Import, &drop_sig)
        .unwrap();
    let drop_local = self.module
        .declare_func_in_func(drop_callee, self.builder.func);
    self.builder.ins().call(drop_local, &[val]);
}

/// 根据元素类型返回对应的 drop 函数名
fn drop_func_for(&self, elem_ty: &FrontendType) -> &'static str {
    match elem_ty {
        FrontendType::I64 | FrontendType::I32 | FrontendType::I16 | FrontendType::I8
            => "array_drop",
        FrontendType::F64 => "array_drop_f64",
        FrontendType::Complex128 => "array_drop_complex128",
        _ => "array_drop",
    }
}
```

### 3.3 `translate_expr` 新增分支

```rust
Expr::Block(body) => {
    self.scope_depth += 1;
    for stmt in body {
        self.translate_expr(stmt);
    }
    // 作用域退出时 auto-drop
    self.emit_scope_drop(self.scope_depth);
    self.scope_depth -= 1;
    // Block 是语句级构造，返回 0（i64 占位值）
    self.builder.ins().iconst(types::I64, 0)
}
```

紧跟 `Expr::IfElse` / `Expr::WhileLoop` 之后。

### 3.4 `emit_scope_drop`：作用域退出时的 auto-drop

```rust
/// 对指定作用域深度中所有未显式 drop 的 DynamicArray 发射 drop 调用
fn emit_scope_drop(&mut self, depth: usize) {
    if let Some(vars) = self.scope_analysis.scope_vars.get(&depth) {
        for name in vars {
            // 查找变量在 variables 表中的 (Variable, FrontendType)
            if let Some((var, ty)) = self.variables.get(name) {
                if self.explicitly_dropped.contains(var) {
                    continue;
                }
                if let FrontendType::DynamicArray(elem_ty) = ty {
                    let drop_func = self.drop_func_for(elem_ty);
                    let val = self.builder.use_var(*var);
                    self.emit_drop_call(drop_func, val);
                }
            }
        }
    }
}
```

### 3.5 `translate` 主函数修改

```rust
fn translate(...) -> Result<(), String> {
    // ... 现有序言（签名、入口块、declare_variables）不变 ...

    let mut trans = FunctionTranslator {
        builder,
        variables,
        module: &mut self.module,
        current_func_name: name,
        current_func_ret_type: to_cranelift_type(&the_return.1),
        string_counter: 0,
        type_checker: &self.type_checker,
        scope_analysis,      // ← 来自 ownership checker
        scope_depth: 0,
        explicitly_dropped: Vec::new(),
    };

    // 逐条翻译
    for expr in stmts {
        trans.translate_expr(expr);
    }

    // 返回变量处理
    let (return_variable, _) = trans.variables.get(&the_return.0)
        .expect("return variable not defined");
    let return_value = trans.builder.use_var(*return_variable);

    // 函数体顶层作用域的 auto-drop（scope_depth=0）
    trans.emit_scope_drop(0);

    trans.builder.ins().return_(&[return_value]);
    trans.builder.finalize();
    Ok(())
}
```

### 3.6 `translate_while_loop` 修改 —— 修复迭代泄漏

```rust
fn translate_while_loop(&mut self, cond: Expr, body: Vec<Expr>) {
    let header_block = self.builder.create_block();
    let body_block = self.builder.create_block();
    let exit_block = self.builder.create_block();

    // 跳转到循环头
    self.builder.ins().jump(header_block);

    // 循环头：计算条件
    self.builder.switch_to_block(header_block);
    let cond_val = self.translate_expr(cond);
    let cond_bool = self.builder.ins().icmp(
        IntCC::NotEqual, cond_val,
        self.builder.ins().iconst(types::I64, 0),
    );
    self.builder.ins().brif(cond_bool, body_block, exit_block);

    // 循环体（body 为 Vec<Expr>，直接消费，无需 clone）
    self.builder.switch_to_block(body_block);
    for stmt in body {
        self.translate_expr(stmt);
    }

    // ★ v2 关键修复：每次迭代结束时释放循环体内定义的 DynamicArray
    // loop_scope_depth 是在进入循环体前由 translate_expr 递增的 scope_depth
    let loop_scope_depth = self.scope_depth; // 见下方 WhileLoop 分支
    self.emit_scope_drop(loop_scope_depth);

    // 跳回循环头
    self.builder.ins().jump(header_block);

    // 退出
    self.builder.switch_to_block(exit_block);
}
```

`translate_expr` 中 WhileLoop 的处理：

```rust
Expr::WhileLoop(cond, body) => {
    self.scope_depth += 1;
    // loop_scope_depth 在 translate_while_loop 内部使用
    self.translate_while_loop(*cond, body);
    // 循环退出后，scope 内的变量已经在最后一次迭代中释放
    // 不需要再次 emit_scope_drop（已经在每次迭代结束时做过了）
    self.scope_depth -= 1;
}
```

### 3.7 `translate_assign`：无需手动登记作用域

由于 JIT 不再独立维护 `scoped_arrays` 栈，`translate_assign` 不需要手动将变量推入作用域。当 DynamicArray 被赋值时，ownership checker 已经将其记录在 `ScopeAnalysis.scope_vars` 中。JIT 在作用域退出时通过 `emit_scope_drop(depth)` 统一查询。

`translate_assign` 保持原有的翻译逻辑不变，只需移除旧的 `dynamic_arrays.push()` 调用。

### 3.8 `Explicitly_dropped` 的同步清理

`explicitly_dropped` 在翻译过程中被填充（当遇到 `Expr::Drop(name)` 时）。当作用域退出时，该作用域内显式 drop 的变量应该从 `explicitly_dropped` 中移除，避免污染其他作用域（同名变量在不同作用域）。

```rust
fn emit_scope_drop(&mut self, depth: usize) {
    if let Some(vars) = self.scope_analysis.scope_vars.get(&depth) {
        for name in vars {
            if let Some((var, ty)) = self.variables.get(name) {
                if self.explicitly_dropped.contains(var) {
                    // 清理：移除当前作用域的 explicit_dropped 记录
                    self.explicitly_dropped.retain(|v| v != var);
                    continue;
                }
                if let FrontendType::DynamicArray(elem_ty) = ty {
                    let drop_func = self.drop_func_for(elem_ty);
                    let val = self.builder.use_var(*var);
                    self.emit_drop_call(drop_func, val);
                }
            }
        }
    }
}
```

---

## 四、活跃度分析（方案 A）的集成

（与 v1 保持不变，推迟实施）

### 4.1 算法概览

```
输入: 一个作用域内的语句列表 stmts[0..n]
输出: 对每个 DynamicArray 变量，确定其 drop 位置 ≤ 作用域边界

算法:
  1. 反向扫描 stmts[0..n]，记录每个变量的"最后出现位置"
  2. 正向翻译 stmts[0..n]：
     - 每翻译完一条语句 stmts[i]，检查是否有变量在 i 处"死亡"
     - 如果变量在 scope_vars 中且未被显式 drop：
       → 在 stmts[i] 之后立刻插入 drop 调用
       → 将其从 scope_vars 中移除
```

### 4.2 实现位置

作为 `OwnershipChecker` 的一个可选输出。在 `analyze_function` 中，对每个作用域运行活跃度分析，输出 `drop_schedule: HashMap<usize, Vec<String>>`——"语句索引 → 在此处应 drop 的变量列表"。JIT 侧在逐语句循环中检查 `drop_schedule`。

### 4.3 暂不实施的原因

- 活跃度分析对 `if/else` 分支需要 meet-point 分析
- 块作用域 + 循环迭代释放已经覆盖了主要的泄漏场景
- 可以留作后续迭代

---

## 五、架构改进：统一所有权追踪

### 5.1 改进前后对比

```
改进前（v1 / 当前代码）:
  ownership.rs          jit.rs
  ┌──────────┐         ┌──────────────┐
  │ 独立维护   │         │ 独立维护      │
  │ arrays     │   ✗无   │ dynamic_arrays│
  │ errors     │  互通   │ explicitly_   │
  │            │         │ dropped       │
  └──────────┘         └──────────────┘
  风险：两套系统可能得出不一致的结论

改进后（v2）:
  ownership.rs                jit.rs
  ┌──────────────┐   输出    ┌─────────────────┐
  │ OwnershipChecker│ ──────→ │ FunctionTranslator│
  │ 计算 ScopeAnalysis│        │ 消费 ScopeAnalysis │
  │ + errors       │          │ + scope_depth     │
  └──────────────┘           │ + explicitly_dropped│
                              └─────────────────┘
  一套分析，两处消费。JIT 仅维护运行时状态（scope_depth, explicitly_dropped）
```

### 5.2 JIT 编译流程更新

```
.toy source
  │
  ▼
[PEG Parser] → AST
  │
  ▼
[Constant Folding] → Optimized AST
  │
  ▼
[Ownership Checker] → (ScopeAnalysis, Vec<OwnershipError>)
  │                         │
  │                         ▼
  │                   if errors.is_empty() → continue
  │                   else → report & abort
  ▼
[JIT Translator]
  ├── declare_variables (first pass)
  ├── translate_expr (second pass)
  │     ├── uses scope_analysis.scope_vars for auto-drop at scope exit
  │     ├── uses scope_analysis.loop_scopes for per-iteration cleanup
  │     └── populates explicitly_dropped during translation
  └── auto-drop top-level scope → return
  │
  ▼
[Cranelift Codegen] → Native code → Execute
```

---

## 六、逐文件修改清单

```
src/
  frontend.rs
    ├── Expr 枚举: 新增 Block(Vec<Expr>) 变体                      (+1 行)
    ├── PEG statement(): 新增 block_stmt() 调用                     (+1 行)
    └── PEG: 新增 block_stmt 规则（无 \n 限制）                     (+5 行)

  type_checker.rs
    └── infer_type(): 新增 Expr::Block 防御性分支                   (+5 行)

  optimizer.rs
    └── fold_constants(): 新增 Expr::Block 分支                     (+5 行)

  ownership.rs
    ├── 新增 ScopeAnalysis 结构体                                   (+12 行)
    ├── OwnershipChecker: 新增 scope_depth, scope_vars,             (+8 行)
    │   loop_scopes, explicit_drops 字段
    ├── analyze_function(): 返回 (ScopeAnalysis, Vec<Error>)        (+12 行修改)
    ├── 新增 analyze_stmts(), close_scope(),                        (+55 行)
    │   expr_produces_dynarray()
    ├── analyze_expr Assign: 登记到 scope_vars + 覆盖检测           (+18 行)
    ├── analyze_expr WhileLoop: 创建 loop_scope                     (+12 行)
    ├── analyze_expr: 新增 Expr::Block 分支                         (+8 行)
    └── 测试: 新增 7 个测试用例                                     (+85 行)

  jit.rs
    ├── FunctionTranslator:                                         (+4 行修改)
    │   scoped_arrays → scope_analysis + scope_depth
    ├── 新增 emit_drop_call(), drop_func_for(), emit_scope_drop()   (+35 行)
    ├── translate(): 使用 scope_analysis + emit_scope_drop(0)       (+12 行修改)
    ├── translate_expr Block: scope_depth++ / emit_scope_drop / --  (+6 行)
    ├── translate_expr WhileLoop: scope_depth++ / --                (+4 行修改)
    ├── translate_while_loop(): 迭代结束时 emit_scope_drop          (+5 行新增)
    ├── translate_assign(): 移除 dynamic_arrays.push()              (-5 行删除)
    ├── translate_drop(): 改用 emit_drop_call()                     (-25 行, 简化为 3 行)
    ├── 函数末尾 auto-drop 循环: 改用 emit_scope_drop(0)            (-15 行, 简化为 1 行)
    └── declare_variables_in_stmt(): 新增 Expr::Block 分支          (+4 行)

  integration_test.rs
    └── 新增 5 个集成测试                                           (+65 行)

examples/
    └── scope_demo.toy (新文件): 块作用域 + 循环释放完整演示         (+60 行)
```

**总估算**：净增 ~320 行，删除/简化 ~45 行重复代码。

---

## 七、测试计划

### 7.1 单元测试（ownership.rs `#[cfg(test)]`）

| # | 测试 | Toy 代码 | 预期 |
|---|---|---|---|
| 1 | `test_block_valid_drop_at_end` | `fn t() -> (r:i64) { { a = array [1]; r = a[0] } }` | ✅ 无错误 |
| 2 | `test_block_leak` | `fn t() -> (r:i64) { { a = array [1]; } r = 0 }` | ✅ 无错误（块退出时 JIT 自动释放，不视为泄漏。仅顶层 scope_depth=0 的未处理 Owned 数组才报泄漏） |
| 3 | `test_nested_block` | `fn t() -> (r:i64) { { { a = array [1]; } } r = 0 }` | ✅ 无错误 |
| 4 | `test_reassign_leak` | `fn t() -> (r:i64) { a = array [1]; a = array [2]; r = 0 }` | ❌ `LeakedArray`（覆盖旧值） |
| 5 | `test_block_after_drop_no_double` | `fn t() -> (r:i64) { a = array [1]; drop(a); }` | ✅ 无错误 |
| 6 | `test_while_loop_scope` | `fn t() -> (r:i64) { i=0; while i<3 { a=array[1]; i=i+1 } r=0 }` | ✅ 无错误（循环体内数组在每次迭代释放） |
| 7 | `test_while_loop_nested_block` | `fn t() -> (r:i64) { i=0; while i<3 { { a=array[1]; } i=i+1 } r=0 }` | ✅ 无错误（循环内的嵌套块同样正确释放） |

### 7.2 集成测试（`tests/integration_test.rs`）

| # | 测试 | 验证点 |
|---|---|---|
| 1 | `test_block_scope_basic` | 块内创建数组 → 块外不可见，运行时正确释放 |
| 2 | `test_nested_block_scope` | 嵌套块，内层数组先释放 |
| 3 | `test_block_with_if` | 块内 if/else 分支中各自创建数组，块退出时全部释放 |
| 4 | `test_while_loop_no_leak` | 循环 1000 次每次创建数组，验证无内存泄漏（通过长时间运行观察） |
| 5 | `test_while_loop_last_iteration` | 循环体内数组在每次迭代后正确释放，最后一次迭代后仍可访问循环外的数组 |

### 7.3 示例脚本（`examples/scope_demo.toy`）

```toy
fn main() -> (r: i64) {
    puts("=== Scope Demo ===\n")

    puts("[1] Top-level array (auto-drop at function end)\n")
    a = array [1, 2, 3]
    printf("  a[0] = %d\n", a[0])

    puts("[2] Block-scoped array (auto-drop at block end)\n")
    {
        b = array [4, 5, 6]
        printf("  b[0] = %d\n", b[0])
    }

    puts("[3] Nested block scopes\n")
    {
        puts("  outer block\n")
        {
            c = array [7, 8, 9]
            printf("  c[0] = %d\n", c[0])
        }
        puts("  c already freed here\n")
    }

    puts("[4] Explicit drop() still works anywhere\n")
    {
        d = array [10, 20]
        printf("  d[0] = %d\n", d[0])
        drop(d)
        puts("  d explicitly dropped inside block\n")
    }

    puts("[5] While loop — arrays created each iteration are released each iteration\n")
    i = 0
    while i < 5 {
        tmp = array [i, i+1]
        printf("  tmp[0] = %d\n", tmp[0])
        i = i + 1
    }

    r = 0
}
```

---

## 八、风险与注意事项

| # | 风险 | 严重程度 | 缓解措施 |
|---|---|---|---|
| 1 | `ScopeAnalysis` 序列化/反序列化 | 低 | `ScopeAnalysis` 仅在编译流程内部使用，不跨进程传递。无需 `Serialize`/`Deserialize` |
| 2 | `emit_drop_call` 每次调用都 `declare_function` | 低 | Cranelift 内部会去重相同签名的函数声明，实际开销可忽略 |
| 3 | `if`/`else` 分支内数组泄漏检测不完整 | 低 | 保守策略，与现有行为一致。增强策略留作后续 |
| 4 | `expr_produces_dynarray` 可能漏判 | 中 | 仅匹配明确的构造形式。如果后续添加新的 DynamicArray 构造方式，需要同步更新该方法。可考虑未来改为调用 `type_checker::infer_type` |
| 5 | 循环内数组的 `explicit_drop` 同步清理 | 中 | 每次迭代结束后清理 `explicitly_dropped` 中的循环作用域变量（见 3.8 节）。需要确保 `drop()` 调用在 `emit_scope_drop` 之前被正确记录 |
| 6 | 现有测试回归 | 低 | 所有现有 `.toy` 脚本不使用 `{ }` 块，行为应完全不变 |
| 7 | `r = arr` 悬垂指针 | 低 | 不在此方案中修复，延续现有行为 |

---

## 九、实施顺序

```
Phase 1（约 3 小时）: AST + 解析 + 所有递归遍历
  1. frontend.rs: 新增 Block 变体 + PEG block_stmt 规则（无 \n 限制）
  2. type_checker.rs: infer_type 新增 Block 防御分支
  3. optimizer.rs: fold_constants 新增 Block 分支
  4. declare_variables_in_stmt: 新增 Block 分支
  5. cargo check — 确保编译通过

Phase 2（约 5 小时）: 所有权检查器改造（输出 ScopeAnalysis）
  6. ownership.rs: 定义 ScopeAnalysis 结构体
  7. ownership.rs: OwnershipChecker 新增字段（scope_depth, scope_vars,
     loop_scopes, explicit_drops）
  8. ownership.rs: 实现 analyze_stmts, close_scope, expr_produces_dynarray
  9. ownership.rs: WhileLoop 创建 loop_scope
  10. ownership.rs: analyze_function 返回 (ScopeAnalysis, Vec<Error>)
  11. ownership.rs: 覆盖泄漏检测
  12. cargo test — 运行所有权单元测试

Phase 3（约 5 小时）: JIT 代码生成改造（消费 ScopeAnalysis）
  13. jit.rs: FunctionTranslator 字段变更（scope_analysis + scope_depth）
  14. jit.rs: emit_drop_call + drop_func_for + emit_scope_drop 实现
  15. jit.rs: translate 修改（初始化 scope_analysis，调用 emit_scope_drop(0)）
  16. jit.rs: translate_expr Block 分支（scope_depth++/emit_scope_drop/--）
  17. jit.rs: translate_while_loop 修改（迭代结束时 emit_scope_drop）
  18. jit.rs: translate_assign 移除 dynamic_arrays.push()
  19. jit.rs: translate_drop 改用 emit_drop_call()
  20. cargo check + 手动测试基础块作用域 + 循环释放

Phase 4（约 3 小时）: 集成验证
  21. 所有权单元测试（7 个）
  22. 集成测试（5 个）
  23. scope_demo.toy 示例脚本
  24. 确保 cargo test --test integration_test 全部通过
  25. cargo run --bin toy -- examples/scope_demo.toy
  26. 更新 PROJECT_GUIDE.md 文档
```

---

## 十、实施偏差记录

### 10.1 Block 在 expression() 而非 statement()

**计划**：`block_stmt` 放在 `statement()` 规则中，实现语句级 Block。

**实际**：放在 `expression()` 规则中，与 `if_else`/`while_loop` 同级放置。

**原因**：`statement()` 返回 `Option<Expr>`（用于处理空行），改变其签名需要联动修改 `statements()` 的 flatten 逻辑。放在 `expression()` 避免了不必要的破坏性变动，且语义上等价——`statement()` 调用 `expression()`，Block 的行为不变。

**影响**：Block 理论上可以出现在表达式位置（如 `x = { ... }`），但实际使用中只作为语句出现，无副作用。

### 10.2 PEG 保留 `"\n"` 前缀

**计划**：移除 `block_stmt` 中 `{` 后的 `"\n"` 要求。

**实际**：保留 `"{" _ "\n"` 前缀。

**原因**：Toy 语法以 `\n` 作为语句终止符（`statement()` 要求 `"\n"` 结尾），`if`/`while`/`function` 的 `{` 后也都要求 `"\n"`。移除会导致 `{ stmt }` 单行写法语句终止符缺失而解析失败。保留 `"\n"` 保持了一致性，且对所有现有 Toy 代码无影响。

### 10.3 `loop_scopes` / `explicit_drops` 从 ScopeAnalysis 中移除

**计划**：`ScopeAnalysis` 包含 `loop_scopes` 和 `explicit_drops` 字段供 JIT 使用。

**实际**：实施中发现这两个字段在 JIT 侧从未被消费。

- `loop_scopes`：循环迭代释放通过 `translate_while_loop` 中直接调用 `emit_scope_drop` 实现，不需要查表
- `explicit_drops`：JIT 使用自己的 `explicitly_dropped: Vec<Variable>`（翻译时填充），所有权检查器的 `explicit_drops` 是编译期信息，无消费者

为保持代码简洁，移除了这两个死字段，`ScopeAnalysis` 仅保留 `scope_vars`。

### 10.4 `test_block_leak` 预期修正

**计划**：块内创建数组但未 drop → 报 `LeakedArray`。

**实际**：嵌套作用域（depth>0）的 Owned 数组由 JIT auto-drop 自动释放，不报泄漏。仅顶层（depth=0）的未处理数组报 `LeakedArray`。

**原因**：RAII 语义下，块退出时自动释放是正常行为。将块内的未使用数组视为"泄漏"会阻止合法代码编译（如创建临时数组只需使用一次即丢弃）。

### 10.5 实际代码量远低于估算

**计划估算**：净增 ~320 行，删除 ~45 行。

**实际**：净增 ~190 行。偏差主要来自：
- `ScopeAnalysis` 字段简化（移除 `loop_scopes`/`explicit_drops`）
- `translate_drop` 重构比预期更精简
- 计划中注释/文档行计入估算，实际注释更紧凑
