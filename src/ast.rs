// src/ast.rs

/// 二元操作符枚举

pub enum Op {
    Add, // +
    Sub, // -
    Mul, // *
    Div, // /
}

/// 表达式节点：代表产生值的计算单元

pub enum Expr {
    /// 字面量整数，例如 42
    Literal(i64),
    /// 变量标识符，例如 x
    Identifier(String),
    /// 二元运算，例如 a + b
    BinaryOp {
        op: Op,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    /// 变量赋值表达式，例如 x = 10 (在 RustScript 中赋值也是表达式，返回被赋的值)
    Assign {
        name: String,
        value: Box<Expr>,
    },
}

/// 语句节点：代表执行动作的单元
pub enum Stmt {
    /// 表达式语句，例如 "1+2;"
    Expr(Expr),
}