//! DynamicArray 所有权检查器
//!
//! 编译期检测 DynamicArray 的泄漏和使用后-drop 等错误。

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
#[derive(Debug, Clone)]
pub struct ScopeAnalysis {
    /// scope_depth -> 该作用域内定义的 DynamicArray 变量名列表
    /// scope_depth=0 为函数体顶层
    pub scope_vars: HashMap<usize, Vec<String>>,
}

/// 所有权检查器，把整个函数体（AST 节点列表）过一遍，对每个 Expr 做状态追踪和违规检测，最后返回发现的错误列表
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

                // 赋值给返回变量：源数组所有权转移给调用者
                if name == return_var {
                    self.arrays.insert(
                        name.clone(),
                        (ArrayInfo {
                            disposition: ArrayDisposition::Returned,
                            name: name.clone(),
                        }, self.scope_depth),
                    );
                    // 源数组标记为 Returned
                    if let Expr::Identifier(src_name) = value.as_ref()
                        && let Some((info, _)) = self.arrays.get_mut(src_name)
                    {
                        info.disposition = ArrayDisposition::Returned;
                    }
                    // FIXME: 与 JIT auto-drop 信息断层，见 jit.rs
                } else if produces_array {
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
                    self.scope_vars
                        .entry(self.scope_depth)
                        .or_default()
                        .push(name.clone());
                }

                // 递归分析 RHS 子表达式
                self.analyze_expr(value, return_var);
            }

            Expr::Drop(name) => {
                self.mark_dropped(name);
            }

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

            Expr::IfElse(cond, then_body, else_body) => {
                self.analyze_expr(cond, return_var);
                for stmt in then_body {
                    self.analyze_expr(stmt, return_var);
                }
                for stmt in else_body {
                    self.analyze_expr(stmt, return_var);
                }
                // 保守策略：不做跨分支 meet-point 分析
            }

            // While 循环：体作为独立作用域，每次迭代结束时由 JIT 释放
            Expr::WhileLoop(cond, body) => {
                self.analyze_expr(cond, return_var);
                self.scope_depth += 1;
                self.scope_vars.insert(self.scope_depth, Vec::new());
                self.analyze_stmts(body, return_var);
                self.close_scope(self.scope_depth);
                self.scope_depth -= 1;
            }

            // Block：嵌套作用域
            Expr::Block(body) => {
                self.scope_depth += 1;
                self.scope_vars.insert(self.scope_depth, Vec::new());
                self.analyze_stmts(body, return_var);
                self.close_scope(self.scope_depth);
                self.scope_depth -= 1;
            }

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
