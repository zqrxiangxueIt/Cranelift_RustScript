//! 常量折叠优化 Pass
//!
//! 在 AST 层面计算编译时可确定的常量表达式，避免运行时的冗余计算。

use crate::frontend::{Expr, Type};

/// 对函数体中的所有语句应用常量折叠优化
pub fn fold_constants_in_stmts(stmts: Vec<Expr>) -> Vec<Expr> {
    stmts.into_iter().map(fold_constants).collect()
}

/// 对单个表达式应用常量折叠优化
pub fn fold_constants(expr: Expr) -> Expr {
    match expr {
        // 算术运算
        Expr::Add(lhs, rhs) => fold_binary_op(*lhs, *rhs, OpType::Add, |a, b| a + b),
        Expr::Sub(lhs, rhs) => fold_binary_op(*lhs, *rhs, OpType::Sub, |a, b| a - b),
        Expr::Mul(lhs, rhs) => fold_binary_op(*lhs, *rhs, OpType::Mul, |a, b| a * b),
        Expr::Div(lhs, rhs) => fold_binary_op(*lhs, *rhs, OpType::Div, |a, b| a / b),

        // 比较运算
        Expr::Eq(lhs, rhs) => fold_cmp(*lhs, *rhs, |a, b| a == b),
        Expr::Ne(lhs, rhs) => fold_cmp(*lhs, *rhs, |a, b| a != b),
        Expr::Lt(lhs, rhs) => fold_cmp(*lhs, *rhs, |a, b| a < b),
        Expr::Le(lhs, rhs) => fold_cmp(*lhs, *rhs, |a, b| a <= b),
        Expr::Gt(lhs, rhs) => fold_cmp(*lhs, *rhs, |a, b| a > b),
        Expr::Ge(lhs, rhs) => fold_cmp(*lhs, *rhs, |a, b| a >= b),

        // 赋值语句
        Expr::Assign(name, val) => Expr::Assign(name, Box::new(fold_constants(*val))),

        // 条件分支 - 递归处理
        Expr::IfElse(cond, then_body, else_body) => Expr::IfElse(
            Box::new(fold_constants(*cond)),
            then_body.into_iter().map(fold_constants).collect(),
            else_body.into_iter().map(fold_constants).collect(),
        ),

        // While 循环 - 递归处理
        Expr::WhileLoop(cond, body) => Expr::WhileLoop(
            Box::new(fold_constants(*cond)),
            body.into_iter().map(fold_constants).collect(),
        ),

        // 函数调用 - 递归处理参数
        Expr::Call(name, args) => Expr::Call(
            name,
            args.into_iter().map(fold_constants).collect(),
        ),

        // 数组索引
        Expr::Index(base, idx) => Expr::Index(
            Box::new(fold_constants(*base)),
            Box::new(fold_constants(*idx)),
        ),

        // 类型转换
        Expr::Cast(expr, ty) => fold_cast(*expr, ty),

        // 字面量、标识符、全局地址等保持不变
        _ => expr,
    }
}

/// 二元运算常量折叠
fn fold_binary_op<F>(
    lhs: Expr,
    rhs: Expr,
    op_type: OpType,
    int_op: F,
) -> Expr
where
    F: Fn(i64, i64) -> i64,
{
    let lhs = Box::new(fold_constants(lhs));
    let rhs = Box::new(fold_constants(rhs));

    match op_type {
        OpType::Add => fold_add(lhs, rhs, int_op),
        OpType::Mul => fold_mul(lhs, rhs, int_op),
        OpType::Sub => fold_sub(lhs, rhs, int_op),
        OpType::Div => fold_div(lhs, rhs, int_op),
    }
}

enum OpType {
    Add,
    Mul,
    Sub,
    Div,
}

/// 加法常量折叠
fn fold_add<F>(lhs: Box<Expr>, rhs: Box<Expr>, int_op: F) -> Expr
where
    F: Fn(i64, i64) -> i64,
{
    match (lhs.as_ref(), rhs.as_ref()) {
        // 0 + x = x
        (Expr::Literal(v, t), r) if is_zero(v, t) => (*r).clone(),
        // x + 0 = x
        (l, Expr::Literal(v, t)) if is_zero(v, t) => (*l).clone(),
        // 两个整数常量
        (Expr::Literal(v1, Type::I64), Expr::Literal(v2, Type::I64)) => {
            if let (Ok(a), Ok(b)) = (v1.parse::<i64>(), v2.parse::<i64>()) {
                Expr::Literal(int_op(a, b).to_string(), Type::I64)
            } else {
                Expr::Add(lhs, rhs)
            }
        }
        // 无法折叠
        _ => Expr::Add(lhs, rhs),
    }
}

/// 乘法常量折叠
fn fold_mul<F>(lhs: Box<Expr>, rhs: Box<Expr>, int_op: F) -> Expr
where
    F: Fn(i64, i64) -> i64,
{
    match (lhs.as_ref(), rhs.as_ref()) {
        // 0 * x = 0
        (Expr::Literal(v, t), _) if is_zero(v, t) => Expr::Literal("0".to_string(), Type::I64),
        // x * 0 = 0
        (_, Expr::Literal(v, t)) if is_zero(v, t) => Expr::Literal("0".to_string(), Type::I64),
        // 1 * x = x
        (Expr::Literal(v, t), r) if is_one(v, t) => (*r).clone(),
        // x * 1 = x
        (l, Expr::Literal(v, t)) if is_one(v, t) => (*l).clone(),
        // 两个整数常量
        (Expr::Literal(v1, Type::I64), Expr::Literal(v2, Type::I64)) => {
            if let (Ok(a), Ok(b)) = (v1.parse::<i64>(), v2.parse::<i64>()) {
                Expr::Literal(int_op(a, b).to_string(), Type::I64)
            } else {
                Expr::Mul(lhs, rhs)
            }
        }
        // 无法折叠
        _ => Expr::Mul(lhs, rhs),
    }
}

/// 减法常量折叠
fn fold_sub<F>(lhs: Box<Expr>, rhs: Box<Expr>, int_op: F) -> Expr
where
    F: Fn(i64, i64) -> i64,
{
    match (lhs.as_ref(), rhs.as_ref()) {
        // x - 0 = x
        (l, Expr::Literal(v, t)) if is_zero(v, t) => (*l).clone(),
        // 两个整数常量
        (Expr::Literal(v1, Type::I64), Expr::Literal(v2, Type::I64)) => {
            if let (Ok(a), Ok(b)) = (v1.parse::<i64>(), v2.parse::<i64>()) {
                Expr::Literal(int_op(a, b).to_string(), Type::I64)
            } else {
                Expr::Sub(lhs, rhs)
            }
        }
        // 无法折叠
        _ => Expr::Sub(lhs, rhs),
    }
}

/// 除法常量折叠 (需要特殊处理除零)
fn fold_div<F>(lhs: Box<Expr>, rhs: Box<Expr>, int_op: F) -> Expr
where
    F: Fn(i64, i64) -> i64,
{
    match (lhs.as_ref(), rhs.as_ref()) {
        // x / 1 = x
        (l, Expr::Literal(v, t)) if is_one(v, t) => (*l).clone(),

        // 0 / x = 0 (x != 0)
        (Expr::Literal(v, t), _) if is_zero(v, t) => (*lhs).clone(),

        // 两个整数常量
        (Expr::Literal(v1, Type::I64), Expr::Literal(v2, Type::I64)) => {
            if let (Ok(a), Ok(b)) = (v1.parse::<i64>(), v2.parse::<i64>()) {
                if b != 0 {
                    let result = int_op(a, b);
                    return Expr::Literal(result.to_string(), Type::I64);
                }
            }
            Expr::Div(lhs, rhs)
        }

        // 两个浮点常量
        (Expr::Literal(v1, Type::F64), Expr::Literal(v2, Type::F64)) => {
            if let (Ok(a), Ok(b)) = (v1.parse::<f64>(), v2.parse::<f64>()) {
                if b != 0.0 && !b.is_nan() {
                    let result = a / b;
                    return Expr::Literal(result.to_string(), Type::F64);
                }
            }
            Expr::Div(lhs, rhs)
        }

        _ => Expr::Div(lhs, rhs),
    }
}

/// 比较运算常量折叠
fn fold_cmp<F>(lhs: Expr, rhs: Expr, cmp: F) -> Expr
where
    F: Fn(i64, i64) -> bool,
{
    let lhs = Box::new(fold_constants(lhs));
    let rhs = Box::new(fold_constants(rhs));

    match (lhs.as_ref(), rhs.as_ref()) {
        // x == x = true
        // x != x = false
        // x < x = false
        // x > x = false
        (Expr::Identifier(n1), Expr::Identifier(n2)) if n1 == n2 => {
            // 比较运算结果是 i64 (0 或 1)
            let result = cmp(0, 0); // 用 0,0 调用 cmp 来获取默认值
            let val = if result { 1 } else { 0 };
            return Expr::Literal(val.to_string(), Type::I64);
        }

        // 两个整数常量: 折叠
        (Expr::Literal(v1, Type::I64), Expr::Literal(v2, Type::I64)) => {
            if let (Ok(a), Ok(b)) = (v1.parse::<i64>(), v2.parse::<i64>()) {
                let result = cmp(a, b);
                Expr::Literal((if result { 1 } else { 0 }).to_string(), Type::I64)
            } else {
                Expr::Eq(lhs, rhs)
            }
        }

        // 无法折叠
        _ => Expr::Eq(lhs, rhs),
    }
}

/// 类型转换常量折叠
fn fold_cast(expr: Expr, target_ty: Type) -> Expr {
    let expr = Box::new(fold_constants(expr));

    match (expr.as_ref(), &target_ty) {
        // 字面量之间的转换: 直接计算
        (Expr::Literal(v, Type::I64), Type::F64) => {
            if let Ok(n) = v.parse::<i64>() {
                Expr::Literal((n as f64).to_string(), Type::F64)
            } else {
                Expr::Cast(expr, target_ty)
            }
        }
        (Expr::Literal(v, Type::F64), Type::I64) => {
            if let Ok(n) = v.parse::<f64>() {
                Expr::Literal((n as i64).to_string(), Type::I64)
            } else {
                Expr::Cast(expr, target_ty)
            }
        }
        (Expr::Literal(v, Type::I64), Type::I32) => {
            if let Ok(n) = v.parse::<i64>() {
                Expr::Literal((n as i32).to_string(), Type::I32)
            } else {
                Expr::Cast(expr, target_ty)
            }
        }
        (Expr::Literal(v, Type::I32), Type::I64) => {
            if let Ok(n) = v.parse::<i32>() {
                Expr::Literal((n as i64).to_string(), Type::I64)
            } else {
                Expr::Cast(expr, target_ty)
            }
        }

        // 无法折叠
        _ => Expr::Cast(expr, target_ty),
    }
}

/// 判断字面量是否为零
fn is_zero(val: &str, ty: &Type) -> bool {
    match ty {
        Type::I64 => val.parse::<i64>().map(|v| v == 0).unwrap_or(false),
        Type::I32 => val.parse::<i32>().map(|v| v == 0).unwrap_or(false),
        Type::I16 => val.parse::<i16>().map(|v| v == 0).unwrap_or(false),
        Type::I8 => val.parse::<i8>().map(|v| v == 0).unwrap_or(false),
        Type::F64 => val.parse::<f64>().map(|v| v == 0.0).unwrap_or(false),
        Type::F32 => val.parse::<f32>().map(|v| v == 0.0).unwrap_or(false),
        _ => false,
    }
}

/// 判断字面量是否为一
fn is_one(val: &str, ty: &Type) -> bool {
    match ty {
        Type::I64 => val.parse::<i64>().map(|v| v == 1).unwrap_or(false),
        Type::I32 => val.parse::<i32>().map(|v| v == 1).unwrap_or(false),
        Type::I16 => val.parse::<i16>().map(|v| v == 1).unwrap_or(false),
        Type::I8 => val.parse::<i8>().map(|v| v == 1).unwrap_or(false),
        Type::F64 => val.parse::<f64>().map(|v| v == 1.0).unwrap_or(false),
        Type::F32 => val.parse::<f32>().map(|v| v == 1.0).unwrap_or(false),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fold_add_constants() {
        let expr = Expr::Add(
            Box::new(Expr::Literal("1".to_string(), Type::I64)),
            Box::new(Expr::Literal("2".to_string(), Type::I64)),
        );
        let result = fold_constants(expr);
        assert_eq!(result, Expr::Literal("3".to_string(), Type::I64));
    }

    #[test]
    fn test_fold_add_with_zero() {
        let expr = Expr::Add(
            Box::new(Expr::Identifier("x".to_string())),
            Box::new(Expr::Literal("0".to_string(), Type::I64)),
        );
        let result = fold_constants(expr);
        assert_eq!(result, Expr::Identifier("x".to_string()));
    }

    #[test]
    fn test_fold_mul_with_one() {
        // 1 * y = y
        let expr = Expr::Mul(
            Box::new(Expr::Literal("1".to_string(), Type::I64)),
            Box::new(Expr::Identifier("y".to_string())),
        );
        let result = fold_constants(expr);
        assert_eq!(result, Expr::Identifier("y".to_string()));
    }

    #[test]
    fn test_fold_mul_with_one_rhs() {
        // y * 1 = y
        let expr = Expr::Mul(
            Box::new(Expr::Identifier("y".to_string())),
            Box::new(Expr::Literal("1".to_string(), Type::I64)),
        );
        let result = fold_constants(expr);
        assert_eq!(result, Expr::Identifier("y".to_string()));
    }

    #[test]
    fn test_fold_mul_with_zero() {
        let expr = Expr::Mul(
            Box::new(Expr::Literal("0".to_string(), Type::I64)),
            Box::new(Expr::Identifier("z".to_string())),
        );
        let result = fold_constants(expr);
        assert_eq!(result, Expr::Literal("0".to_string(), Type::I64));
    }

    #[test]
    fn test_nested_fold() {
        // (1 + 2) + (3 + 4) -> 3 + 7 -> 10
        let expr = Expr::Add(
            Box::new(Expr::Add(
                Box::new(Expr::Literal("1".to_string(), Type::I64)),
                Box::new(Expr::Literal("2".to_string(), Type::I64)),
            )),
            Box::new(Expr::Add(
                Box::new(Expr::Literal("3".to_string(), Type::I64)),
                Box::new(Expr::Literal("4".to_string(), Type::I64)),
            )),
        );
        let result = fold_constants(expr);
        assert_eq!(result, Expr::Literal("10".to_string(), Type::I64));
    }
}
