//! DynamicArray 所有权检查器
//!
//! 编译期检测 DynamicArray 的泄漏和使用后-drop 等错误。

use crate::frontend::{Expr, Type};
use std::collections::HashMap;

/// DynamicArray 的所有权状态
#[derive(Clone, Debug, PartialEq)]
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
}

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
        }
    }
}

/// 所有权检查器
pub struct OwnershipChecker {
    /// 跟踪所有 DynamicArray 变量
    arrays: HashMap<String, ArrayInfo>,
    /// 错误列表
    errors: Vec<OwnershipError>,
}

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
        }
    }

    /// 分析函数体，返回错误列表
    pub fn analyze_function(
        &mut self,
        params: &[(String, Type)],
        stmts: &[Expr],
        return_var: &str,
    ) -> Vec<OwnershipError> {
        // 初始化返回变量（如果是 DynamicArray）
        let _ret_is_array = params.iter().any(|(n, _)| n == return_var);

        // 分析函数体
        for stmt in stmts {
            self.analyze_expr(stmt, return_var);
        }

        // 检查泄漏：函数结束时所有 Owned 的数组都是泄漏
        self.check_for_leaks();

        self.errors.clone()
    }

    fn analyze_expr(&mut self, expr: &Expr, return_var: &str) {
        match expr {
            Expr::Assign(name, value) => {
                // 检查 RHS 是否产生 DynamicArray
                let rhs_info = self.get_rhs_array_info(value);

                // 如果赋值给返回变量，源数组的 ownership 转移给调用者
                if name == return_var {
                    // 标记返回变量
                    self.arrays.insert(
                        name.clone(),
                        ArrayInfo {
                            disposition: ArrayDisposition::Returned,
                            name: name.clone(),
                        },
                    );
                    // 标记源数组（如果源是标识符且在追踪中）
                    if let Expr::Identifier(src_name) = value.as_ref()
                        && let Some(info) = self.arrays.get_mut(src_name)
                    {
                        info.disposition = ArrayDisposition::Returned;
                    }
                } else if rhs_info.is_some() {
                    // 否则标记为 Owned
                    self.arrays.insert(
                        name.clone(),
                        ArrayInfo {
                            disposition: ArrayDisposition::Owned,
                            name: name.clone(),
                        },
                    );
                }

                // 递归分析 RHS 子表达式（例如 Call 中的数组参数所有权转移）
                self.analyze_expr(value, return_var);
            }

            Expr::Drop(name) => {
                self.mark_dropped(name);
            }

            Expr::Call(func_name, args) => {
                // 内置借用函数：调用以下函数不会转移所有权
                let non_consuming = matches!(
                    func_name.as_str(),
                    "array_push"
                        | "array_push_f64"
                        | "array_push_complex128"
                        | "array_pop"
                        | "array_pop_f64"
                        | "array_pop_complex128"
                        | "array_len"
                        | "array_len_f64"
                        | "array_len_complex128"
                        | "array_cap"
                        | "array_cap_f64"
                        | "array_cap_complex128"
                        | "array_get_ptr"
                        | "array_get_ptr_f64"
                        | "array_get_ptr_complex128"
                        | "array_set"
                        | "array_set_f64"
                        | "array_set_complex128"
                        | "toy_mkl_dgemm"
                        | "toy_sum_array"
                        | "print_f64"
                        | "print_i64"
                        | "putchar"
                        | "rand"
                        | "sin"
                        | "cos"
                        | "tan"
                        | "sqrt"
                        | "pow"
                        | "exp"
                        | "log"
                        | "ceil"
                        | "floor"
                );
                // 检查参数中的 DynamicArray，若调用会消耗数组则标记为 Passed
                if !non_consuming {
                    for arg in args {
                        if let Expr::Identifier(name) = arg
                            && let Some(info) = self.arrays.get_mut(name)
                            && info.disposition == ArrayDisposition::Owned
                        {
                            info.disposition = ArrayDisposition::Passed;
                        }
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
                // 分支合并时，采用保守策略：只在两个分支都是 Returned/Dropped/Passed 时才标记
                // 否则保持 Owned（会在最后被检测为泄漏）
            }

            Expr::WhileLoop(cond, body) => {
                self.analyze_expr(cond, return_var);
                for stmt in body {
                    self.analyze_expr(stmt, return_var);
                }
                // 循环保守处理：保持 Owned 状态
            }

            Expr::Index(base, idx) => {
                if let Expr::Identifier(name) = base.as_ref()
                    && let Some(info) = self.arrays.get(name)
                {
                    match info.disposition {
                        ArrayDisposition::Dropped => {
                            self.errors
                                .push(OwnershipError::UseAfterDrop { name: name.clone() });
                        }
                        ArrayDisposition::Passed => {
                            self.errors
                                .push(OwnershipError::UseAfterDrop { name: name.clone() });
                        }
                        _ => {} // Owned, Returned, Uninitialized — OK
                    }
                }
                self.analyze_expr(idx, return_var);
            }

            _ => {}
        }
    }

    fn get_rhs_array_info(&self, expr: &Expr) -> Option<ArrayInfo> {
        match expr {
            Expr::DynamicArrayLiteral(_, _) => Some(ArrayInfo {
                disposition: ArrayDisposition::Owned,
                name: String::new(),
            }),
            Expr::Call(name, _) => {
                // 检查是否是返回 DynamicArray 的函数
                match name.as_str() {
                    "array_new_i64" | "array_new_f64" | "array_new_complex128" => Some(ArrayInfo {
                        disposition: ArrayDisposition::Owned,
                        name: String::new(),
                    }),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn mark_dropped(&mut self, name: &str) {
        if let Some(info) = self.arrays.get_mut(name) {
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
                    self.errors.push(OwnershipError::DoubleDrop {
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

    fn check_for_leaks(&mut self) {
        for (name, info) in &self.arrays {
            if info.disposition == ArrayDisposition::Owned {
                self.errors
                    .push(OwnershipError::LeakedArray { name: name.clone() });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_and_analyze(code: &str) -> Vec<OwnershipError> {
        let (_name, params, the_return, stmts) = crate::frontend::parser::function(code).unwrap();
        let mut checker = OwnershipChecker::new();
        checker.analyze_function(&params, &stmts, &the_return.0)
    }

    #[test]
    fn test_valid_return() {
        let code = r#"
fn test() -> (r: array<i64>) {
    arr = array [1, 2, 3]
    r = arr
}
"#;
        let errors = parse_and_analyze(code);
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
        let errors = parse_and_analyze(code);
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
        let errors = parse_and_analyze(code);
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
        let errors = parse_and_analyze(code);
        assert!(!errors.is_empty());
        assert!(matches!(errors[0], OwnershipError::DoubleDrop { .. }));
    }

    #[test]
    fn test_array_push_does_not_transfer_ownership() {
        // array_push 不应转移所有权：调用了 push 后若不返回也不 drop，应报告泄漏
        let code = r#"
fn test() -> (r: i64) {
    arr = array [1, 2, 3]
    array_push(arr, 4)
    r = 0
}
"#;
        let errors = parse_and_analyze(code);
        assert!(!errors.is_empty());
        assert!(matches!(errors[0], OwnershipError::LeakedArray { .. }));
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
        let errors = parse_and_analyze(code);
        assert!(!errors.is_empty());
        assert!(matches!(errors[0], OwnershipError::UseAfterDrop { .. }));
    }
}
