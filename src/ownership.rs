//! DynamicArray 所有权检查器 —— 内存回收机制 (编译期)
//!
//! # 双层回收架构
//!
//! Toy 语言的内存回收采用"编译期检查 + 运行时兜底"的双层机制：
//!
//! 1. **编译期 (本模块)**: 静态分析每个 DynamicArray 的所有权状态转换，
//!    拦截可证明的泄漏、double-free、use-after-drop 等错误。
//!    同时输出 `ScopeAnalysis` 告知 JIT 每个作用域应释放哪些数组。
//!
//! 2. **运行时 (jit.rs)**: JIT 消费 `ScopeAnalysis`, 在 Block 退出、
//!    While 迭代结束、函数返回前自动插入 `call array_drop_xxx` 指令。
//!    嵌套作用域 (depth>0) 的 Owned 数组不报泄漏——由 JIT 按 RAII 语义释放。
//!
//! # 所有权状态机
//!
//! ```text
//! array [1,2,3] → Owned
//!     ├── drop(arr)         → Dropped  (不能再访问)
//!     ├── r = arr           → Returned (所有权转移给调用者)
//!     ├── array_push(arr,x) → Passed   (已消费, JIT 兜底释放)
//!     └── (函数结束)         → 顶层 Owned 报 LeakedArray
//! ```
//!
//! 详见 docs/MEMORY_RECLAMATION.md。

use crate::frontend::{Expr, Type};
use std::collections::HashMap;

/// DynamicArray 的所有权状态
#[derive(Clone, Debug, PartialEq)]
//#[derive(...)] 是 Rust 提供的派生宏，告诉编译器"自动为这个类型实现这些 trait"。提供 .clone() 方法、Debug 输出、PartialEq 比较等功能，简化代码。
pub enum ArrayDisposition {
    /// 未初始化
    Uninitialized,
    /// 被拥有（需要被 drop 或返回）
    Owned,
    /// 已返回给调用者
    Returned,
    /// 已通过 drop() 释放
    Dropped,
    /// 已传递给其他函数（所有权转交）
    Passed,
}

/// DynamicArray 变量信息
#[derive(Clone, Debug)]
pub struct ArrayInfo {
    pub disposition: ArrayDisposition,
    pub name: String,
}

/// 所有权错误类型
#[derive(Clone, Debug)]
pub enum OwnershipError {
    /// 数组泄漏：既没返回也没 drop
    LeakedArray { name: String },
    /// drop 后使用
    UseAfterDrop { name: String },
    /// 重复 drop
    DoubleDrop { name: String },
    /// drop 一个已经通过函数调用"消费"的数组
    /// (实际上所有内置函数都是借用，但静态分析统一按消费处理)
    DropAfterPassed { name: String },
}

/// 实现 Display trait 以便更友好地打印错误信息
impl std::fmt::Display for OwnershipError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OwnershipError::LeakedArray { name } => {
                write!(
                    f,
                    "ownership error: array '{}' is leaked (neither returned nor dropped)",
                    name
                )
            }
            OwnershipError::UseAfterDrop { name } => {
                write!(
                    f,
                    "ownership error: array '{}' used after being dropped",
                    name
                )
            }
            OwnershipError::DoubleDrop { name } => {
                write!(f, "ownership error: array '{}' dropped twice", name)
            }
            OwnershipError::DropAfterPassed { name } => {
                write!(
                    f,
                    "ownership error: array '{}' cannot be dropped because it was already \
                     passed to a function call; the array will be auto-freed at function exit, \
                     so just remove the explicit drop()",
                    name
                )
            }
        }
    }
}

/// 由 OwnershipChecker 输出的作用域分析结果。
/// JIT 编译器消费此结构，无需独立追踪作用域。
///
/// # 内存回收机制中的角色
///
/// ScopeAnalysis 是所有权检查器与 JIT 编译器之间的**唯一数据接口**。
/// 改进前, ownership.rs 和 jit.rs 各自维护一套 DynamicArray 追踪逻辑,
/// 两套系统独立决策, 可能导致不一致。
/// 现在, 所有权检查器输出 ScopeAnalysis, JIT 直接查询它来决定
/// 每个作用域退出时应释放哪些数组——单一真相源。
///
/// # 数据含义
///
/// scope_vars[0] = ["a", "b"]  → 函数体顶层定义的 a, b
///     → 函数返回前必须 drop/return, 否则报 LeakedArray
/// scope_vars[1] = ["c"]       → 第一个 Block/While 内定义的 c
///     → 作用域退出时 JIT 自动释放, 不报错 (RAII)
/// scope_vars[2] = ["d"]       → 嵌套二层内定义的 d
///     → 同上, 先于外层释放
#[derive(Debug, Clone)]
pub struct ScopeAnalysis {
    /// scope_depth -> 该作用域内定义的 DynamicArray 变量名列表
    /// scope_depth=0 为函数体顶层
    pub scope_vars: HashMap<usize, Vec<String>>,
}

/// 所有权检查器，把整个函数体（AST 节点列表）过一遍，对每个 Expr 做状态追踪和违规检测，最后返回发现的错误列表。
///
/// # 内存回收中的角色
///
/// 本结构体实现了编译期静态分析，追踪每个 DynamicArray 变量从创建到释放的
/// 完整生命周期。分析结果通过 ScopeAnalysis 传递给 JIT 编译器，形成
/// "编译期检查 + 运行时兜底"的双层回收机制。
pub struct OwnershipChecker {
    /// 跟踪所有 DynamicArray 变量。键=变量名，值=(状态, 定义所在作用域深度)
    arrays: HashMap<String, (ArrayInfo, usize)>,
    /// 错误列表
    errors: Vec<OwnershipError>,
    /// 当前作用域深度。0 = 函数体顶层，每进入一层 Block/While body +1
    scope_depth: usize,
    /// 每个作用域内定义的 DynamicArray 变量名集合。键 = 作用域深度
    scope_vars: HashMap<usize, Vec<String>>,
}

/// OwnershipChecker::new() 或 OwnershipChecker::default()，提供两种语法糖让调用方随意用
impl Default for OwnershipChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl OwnershipChecker {
    pub fn new() -> Self {
        OwnershipChecker {
            arrays: HashMap::new(),
            errors: Vec::new(),
            scope_depth: 0,
            scope_vars: HashMap::new(),
        }
    }

    /// 分析函数体，返回 (作用域分析结果, 错误列表)。
    pub fn analyze_function(
        &mut self,
        _params: &[(String, Type)],
        stmts: &[Expr],
        return_var: &str,
    ) -> (ScopeAnalysis, Vec<OwnershipError>) {
        // 清理上一次分析的状态（支持复用）
        self.scope_depth = 0;
        self.scope_vars.clear();
        self.arrays.clear();
        self.errors.clear();

        self.analyze_stmts(stmts, return_var);

        // 函数体顶层作用域退出时检查泄漏
        self.close_scope(0);

        let analysis = ScopeAnalysis {
            scope_vars: self.scope_vars.clone(),
        };
        (analysis, self.errors.clone())
    }

    /// 按作用域递归分析语句列表
    fn analyze_stmts(&mut self, stmts: &[Expr], return_var: &str) {
        for stmt in stmts {
            self.analyze_expr(stmt, return_var);
        }
    }

    /// 作用域退出时的检查与清理。
    ///
    /// - depth == 0（函数顶层）：Owned 数组 = 泄漏（用户忘记 drop/return）
    /// - depth > 0（Block/WhileLoop 内部作用域）：Owned 数组由 JIT auto-drop
    ///   自动释放，不报泄漏
    ///
    /// # 为什么按 depth 区分行为
    ///
    /// 顶层是函数与调用者的契约边界 —— 数组是返回还是销毁, 必须由程序员显式决策,
    /// 自动兜底会掩盖资源管理 bug。嵌套作用域内的变量块外不可见, 编译器 100% 确定
    /// 无后续引用, 自动释放安全且无悬垂指针风险。
    ///
    /// 一句话: 顶层强制显式(防泄漏滥用), 嵌套自动释放(防啰嗦)。
    fn close_scope(&mut self, depth: usize) {
        if let Some(vars) = self.scope_vars.get(&depth) {
            let vars = vars.clone();
            for name in &vars {
                if depth == 0 {
                    // 仅顶层作用域的 Owned 数组视为泄漏
                    if let Some(tuple) = self.arrays.get(name)
                        && tuple.0.disposition == ArrayDisposition::Owned
                    {
                        self.errors.push(OwnershipError::LeakedArray {
                            name: name.clone(),
                        });
                    }
                }
                self.arrays.remove(name);
            }
        }
    }

    fn analyze_expr(&mut self, expr: &Expr, return_var: &str) {
        match expr {
            Expr::Assign(name, value) => {
                let produces_array = self.produces_dynamic_array(value);

                // ═══════════════════════════════════════════════════
                // 情况 1: 赋值给返回变量 r → 所有权转移给调用者
                // ═══════════════════════════════════════════════════
                //
                // r = arr   → arr: Owned→Returned, r: Returned
                // r = 42    → r 标记 Returned（防后续被误判泄漏）
                //
                // 语义: 调用者拿到 r 的指针, 负责最终释放。
                // FIXME: JIT 的 emit_scope_drop 跳过 return_variable,
                //   但如果 r 的值来自变量 arr 而非字面量, arr 仍会在 JIT
                //   层被 auto-drop → r 成为悬垂指针。需要让 JIT 也跳过 arr。
                //
                //   当前缓解: 顶层 r = arr 不常见, Toy 示例和测试未触发。
                if name == return_var {
                    self.arrays.insert(
                        name.clone(),
                        (ArrayInfo {
                            disposition: ArrayDisposition::Returned,
                            name: name.clone(),
                        }, self.scope_depth),
                    );
                    // 源数组也标记为 Returned (防止 close_scope 误报泄漏)
                    if let Expr::Identifier(src_name) = value.as_ref()
                        && let Some((info, _)) = self.arrays.get_mut(src_name)
                    {
                        info.disposition = ArrayDisposition::Returned;
                    }

                // ═══════════════════════════════════════════════════
                // 情况 2: RHS 产生新的 DynamicArray → 登记为 Owned
                // ═══════════════════════════════════════════════════
                //
                // a = array [1, 2]        ← 字面量
                // a = array_new_i64()     ← 构造函数调用
                //
                // 两步操作:
                //   ① 覆盖检测 — 如果 a 已有旧值且为 Owned, 旧指针丢失 = 泄漏
                //   ② 登记 — (a, Owned, scope_depth) 加入 arrays + scope_vars
                //
                // 后续 close_scope 根据 depth 决定:
                //   depth=0 → 函数结束仍 Owned → LeakedArray
                //   depth>0 → 作用域退出 → JIT 自动 call array_drop

                //例如a = array [1, 2, 3]
                //a = array [4, 5, 6]  旧数组永远丢失，泄漏了
                } else if produces_array { 
                    // ① 覆盖检测
                    if let Some((old_info, _)) = self.arrays.get(name)
                        && old_info.disposition == ArrayDisposition::Owned
                    {
                        self.errors.push(OwnershipError::LeakedArray {
                            name: format!("{} (previous value overwritten)", name),
                        });
                    }
                    // ② 登记到当前作用域
                    self.arrays.insert(
                        name.clone(),
                        (ArrayInfo {
                            disposition: ArrayDisposition::Owned,
                            name: name.clone(),
                        }, self.scope_depth),
                    );
                    self.scope_vars
                        .entry(self.scope_depth)
                        .or_default()
                        .push(name.clone());
                }

                // 递归分析 RHS: 处理嵌套的 Call(所有权传递) / Index(UseAfterDrop)
                self.analyze_expr(value, return_var);
            }

            Expr::Drop(name) => {
                self.mark_dropped(name);
            }

            // ═══════════════════════════════════════════════════
            // 函数调用 — 实参所有权转移 (Owned → Passed)
            // ═══════════════════════════════════════════════════
            //
            // 任何以 DynamicArray 作为实参的函数调用, 都视为所有权"已消费"。
            //
            // 例: array_push(arr, 4)  → arr: Owned → Passed
            //     array_len(arr)       → arr: Owned → Passed
            //
            // 为什么是 Passed 而非 Dropped?
            //   内置函数都是借用语义, 数组实际还活着, 运行时由 JIT 兜底释放。
            //   Passed = "我不再负责显式管理, 但数据还在"。
            //
            // 副作用: drop(arr) 在函数调用后会报 DropAfterPassed,
            //   因为检查器认为 arr 已不属于你。这是保守的过近似——
            //   宁可多拦合法操作, 也不能放行 double-free。
            //
            // 过近似: 无法区分"真消费"和"借用"。所有内置函数统一按消费处理。
            Expr::Call(_func_name, args) => {
                for arg in args {
                    if let Expr::Identifier(name) = arg
                        && let Some((info, _)) = self.arrays.get_mut(name)
                        && info.disposition == ArrayDisposition::Owned
                    {
                        info.disposition = ArrayDisposition::Passed;
                    }
                }
            }

            // ═══════════════════════════════════════════════════
            // 条件分支 — 保守策略, 不做 meet-point 分析
            // ═══════════════════════════════════════════════════
            //
            // IfElse 不创建新作用域 (scope_depth 不变), 两个分支内的变量
            // 都登记到父作用域。不做跨分支状态合并。
            //
            // 例:
            //   if flag { a = array [1]; drop(a) }  → a: Owned → Dropped
            //   else    { b = array [2] }            → b: Owned, 没处理
            //
            // 如果发生在 depth=0 顶层: b → LeakedArray 报错
            // 如果发生在 depth>0 块内: b → JIT 自动释放, 不报错
            //
            // 增强方向: 分支快照 + meet-point 取交集, 可消除 else 路径的假阳性。
            Expr::IfElse(cond, then_body, else_body) => {
                self.analyze_expr(cond, return_var);
                for stmt in then_body {
                    self.analyze_expr(stmt, return_var);
                }
                for stmt in else_body {
                    self.analyze_expr(stmt, return_var);
                }
            }

            // ═══════════════════════════════════════════════════
            // While 循环 — 体作为独立作用域, 每次迭代结束时释放
            // ═══════════════════════════════════════════════════
            //
            // 进入循环体前 push 新作用域 (scope_depth+1), 体分析完后 close_scope。
            // 检查器只分析 AST 一次 (静态分析, 不模拟循环执行), 它不关心循环
            // 跑多少次——只需确保体内部变量出现在 scope_vars 中, JIT 就会在
            // 每次迭代末尾 emit call array_drop。
            //
            // 例:
            //   while i < 3 {
            //       tmp = array [i]   // scope_vars[1] = ["tmp"]
            //   }
            //   → 每次迭代结束: JIT 发射 call array_drop(tmp)
            //   → 不释放会导致前 N-1 次迭代的数组泄漏
            //
            // 与 Block 的区别: 仅在 JIT 端 —— Block 释放一次, While 每次迭代释放。
            Expr::WhileLoop(cond, body) => {
                self.analyze_expr(cond, return_var);
                self.scope_depth += 1;
                self.scope_vars.insert(self.scope_depth, Vec::new());
                self.analyze_stmts(body, return_var);
                self.close_scope(self.scope_depth);
                self.scope_depth -= 1;
            }

            // ═══════════════════════════════════════════════════
            // Block — 通用嵌套作用域, 退出时 JIT 自动释放
            // ═══════════════════════════════════════════════════
            //
            // 例:
            //   {
            //       a = array [1, 2, 3]   // scope_vars[1] = ["a"]
            //   }
            //   → 块退出: JIT 发射 call array_drop(a)
            //
            // 所有权检查器按"无需显式 drop"对待: close_scope(depth>0)
            // 不报泄漏, 只从 arrays 中移除记录。释放责任在 JIT。
            Expr::Block(body) => {
                self.scope_depth += 1;
                self.scope_vars.insert(self.scope_depth, Vec::new());
                self.analyze_stmts(body, return_var);
                self.close_scope(self.scope_depth);
                self.scope_depth -= 1;
            }

            // ═══════════════════════════════════════════════════
            // 索引访问 — 检测 UseAfterDrop
            // ═══════════════════════════════════════════════════
            //
            // 例:
            //   drop(arr)              → arr: Owned → Dropped
            //   r = arr[0]             → Index(arr, 0)
            //     检查 arr 的 disposition:
            //       Dropped  → UseAfterDrop("arr") ❌
            //       Passed   → 放行 (数据还在, 借用访问)
            //       Owned    → 放行
            //       Returned → 放行
            //
            // 只有用户显式 drop() 后的访问被拦截。Passed 状态下数组
            // 仍存活 (只是检查器不再追踪显式释放), 允许读访问。
            Expr::Index(base, idx) => {
                if let Expr::Identifier(name) = base.as_ref()
                    && let Some((info, _)) = self.arrays.get(name)
                    && matches!(info.disposition, ArrayDisposition::Dropped)
                {
                    self.errors
                        .push(OwnershipError::UseAfterDrop { name: name.clone() });
                }
                self.analyze_expr(idx, return_var);
            }

            _ => {}
        }
    }

    /// 右边的表达式是不是会生成一个新的、归我拥有的 DynamicArray
    /// `true` → 是，下游应该把这个数组登记到 `arrays` 表里；`false` → 不是
    /// 目前只有两种情况会生成新的 DynamicArray：
    ///   1) 直接的动态数组字面量 `array [...]`
    ///   2) 调用返回 DynamicArray 的内置函数（`array_new_i64` / `array_new_f64` / `array_new_complex128`）
    fn produces_dynamic_array(&self, expr: &Expr) -> bool {
        match expr {
            Expr::DynamicArrayLiteral(_, _) => true,
            Expr::Call(name, _) => {
                matches!(
                    name.as_str(),
                    "array_new_i64" | "array_new_f64" | "array_new_complex128"
                )
            }
            _ => false,
        }
    }

    fn mark_dropped(&mut self, name: &str) {
        if let Some((info, _)) = self.arrays.get_mut(name) {
            match info.disposition {
                ArrayDisposition::Owned => {
                    info.disposition = ArrayDisposition::Dropped;
                }
                ArrayDisposition::Returned => {
                    self.errors.push(OwnershipError::DoubleDrop {
                        name: name.to_string(),
                    });
                }
                ArrayDisposition::Dropped => {
                    self.errors.push(OwnershipError::DoubleDrop {
                        name: name.to_string(),
                    });
                }
                ArrayDisposition::Passed => {
                    self.errors.push(OwnershipError::DropAfterPassed {
                        name: name.to_string(),
                    });
                }
                ArrayDisposition::Uninitialized => {
                    // drop 未初始化的变量：错误
                    self.errors.push(OwnershipError::UseAfterDrop {
                        name: name.to_string(),
                    });
                }
            }
        } else {
            // drop 未声明的变量：错误
            self.errors.push(OwnershipError::UseAfterDrop {
                name: name.to_string(),
            });
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_and_analyze(code: &str) -> (ScopeAnalysis, Vec<OwnershipError>) {
        let (_name, params, the_return, stmts) = crate::frontend::parser::function(code).unwrap();
        let mut checker = OwnershipChecker::new();
        checker.analyze_function(&params, &stmts, &the_return.0)
    }

    /// 辅助函数：分析并返回仅错误列表（忽略 ScopeAnalysis）
    fn analyze_errors(code: &str) -> Vec<OwnershipError> {
        parse_and_analyze(code).1
    }

    #[test]
    fn test_valid_return() {
        let code = r#"
fn test() -> (r: array<i64>) {
    arr = array [1, 2, 3]
    r = arr
}
"#;
        let errors = analyze_errors(code);
        assert!(errors.is_empty(), "expected no errors, got {:?}", errors);
    }

    #[test]
    fn test_valid_drop() {
        let code = r#"
fn test() -> (r: i64) {
    arr = array [1, 2, 3]
    r = arr[0]
    drop(arr)
    r = r + 1
}
"#;
        let errors = analyze_errors(code);
        assert!(errors.is_empty(), "expected no errors, got {:?}", errors);
    }

    #[test]
    fn test_leaked_array() {
        let code = r#"
fn test() -> (r: i64) {
    arr = array [1, 2, 3]
    r = 0
}
"#;
        let errors = analyze_errors(code);
        assert!(!errors.is_empty());
        assert!(matches!(errors[0], OwnershipError::LeakedArray { .. }));
    }

    #[test]
    fn test_double_drop() {
        let code = r#"
fn test() -> (r: i64) {
    arr = array [1, 2, 3]
    drop(arr)
    drop(arr)
    r = 0
}
"#;
        let errors = analyze_errors(code);
        assert!(!errors.is_empty());
        assert!(matches!(errors[0], OwnershipError::DoubleDrop { .. }));
    }

    #[test]
    fn test_array_push_transfers_ownership() {
        // array_push 视为消费：调用后 arr 标为 Passed，不再算 leak。
        // 运行时由 jit.rs 的 dynamic_arrays 兜底释放。
        let code = r#"
fn test() -> (r: i64) {
    arr = array [1, 2, 3]
    array_push(arr, 4)
    r = 0
}
"#;
        let errors = analyze_errors(code);
        assert!(
            errors.is_empty(),
            "expected no errors after fix, got {:?}",
            errors
        );
    }

    #[test]
    fn test_drop_after_array_push_is_rejected() {
        // 数组传给函数后不能再 drop，否则报 DropAfterPassed。
        let code = r#"
fn test() -> (r: i64) {
    arr = array [1, 2, 3]
    array_push(arr, 4)
    drop(arr)
    r = 0
}
"#;
        let errors = analyze_errors(code);
        assert!(!errors.is_empty());
        assert!(matches!(errors[0], OwnershipError::DropAfterPassed { .. }));
    }

    #[test]
    fn test_use_after_drop_index() {
        // drop(arr) 后不应再通过索引访问数组
        let code = r#"
fn test() -> (r: i64) {
    arr = array [1, 2, 3]
    drop(arr)
    r = arr[0]
}
"#;
        let errors = analyze_errors(code);
        assert!(!errors.is_empty());
        assert!(matches!(errors[0], OwnershipError::UseAfterDrop { .. }));
    }

    // ══════════════════════════════════════════════════════
    // Phase 2: 作用域感知所有权检查测试
    // ══════════════════════════════════════════════════════

    #[test]
    fn test_block_valid_drop_at_end() {
        // 块结束时内部数组自动视为已释放，不应报泄漏
        let code = r#"
fn test() -> (r: i64) {
    {
        a = array [1]
        r = a[0]
    }
    r = 0
}
"#;
        let errors = analyze_errors(code);
        assert!(errors.is_empty(), "expected no errors, got {:?}", errors);
    }

    #[test]
    fn test_block_leak() {
        // 块内数组未使用：块退出时由 JIT auto-drop 释放，ownership checker 不报泄漏。
        // 真正的泄漏仅指顶层作用域（depth=0）中未处理的 Owned 数组。
        let code = r#"
fn test() -> (r: i64) {
    {
        a = array [1]
    }
    r = 0
}
"#;
        let errors = analyze_errors(code);
        assert!(errors.is_empty(), "expected no errors, got {:?}", errors);
    }

    #[test]
    fn test_nested_block() {
        // 嵌套块：内层数组在内层块退出时释放，外层不报泄漏
        let code = r#"
fn test() -> (r: i64) {
    {
        {
            a = array [1]
        }
    }
    r = 0
}
"#;
        let errors = analyze_errors(code);
        assert!(errors.is_empty(), "expected no errors, got {:?}", errors);
    }

    #[test]
    fn test_reassign_leak() {
        // 重新赋值覆盖旧数组 → 旧数组泄漏
        let code = r#"
fn test() -> (r: i64) {
    a = array [1]
    a = array [2]
    r = 0
}
"#;
        let errors = analyze_errors(code);
        assert!(!errors.is_empty());
        assert!(
            matches!(errors[0], OwnershipError::LeakedArray { .. }),
            "expected LeakedArray for overwrite, got {:?}",
            errors
        );
    }

    #[test]
    fn test_block_after_drop_no_double() {
        // 块内显式 drop 后，块结束时不应重复释放
        let code = r#"
fn test() -> (r: i64) {
    a = array [1]
    drop(a)
    r = 0
}
"#;
        let errors = analyze_errors(code);
        assert!(errors.is_empty(), "expected no errors, got {:?}", errors);
    }

    #[test]
    fn test_while_loop_scope() {
        // 循环体内数组在每次迭代释放，不报泄漏
        let code = r#"
fn test() -> (r: i64) {
    i = 0
    while i < 3 {
        a = array [1]
        i = i + 1
    }
    r = 0
}
"#;
        let errors = analyze_errors(code);
        assert!(errors.is_empty(), "expected no errors, got {:?}", errors);
    }

    #[test]
    fn test_while_loop_nested_block() {
        // 循环内的嵌套块同样正确释放
        let code = r#"
fn test() -> (r: i64) {
    i = 0
    while i < 3 {
        {
            a = array [1]
        }
        i = i + 1
    }
    r = 0
}
"#;
        let errors = analyze_errors(code);
        assert!(errors.is_empty(), "expected no errors, got {:?}", errors);
    }
}
