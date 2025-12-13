

use crate::ast::{Expr, Op, Stmt};

peg::parser! {
    pub grammar rustscript_parser() for str {

        // 忽略空白字符
        rule whitespace() = [' ' | '\t' | '\n' | '\r']*

        // 程序由一系列以分号分隔的语句组成
        pub rule program() -> Vec<Stmt>
            = stmts:(statement() ** (";" whitespace())) whitespace() { stmts }

        rule statement() -> Stmt
            = e:expression() { Stmt::Expr(e) }

        pub rule expression() -> Expr
            = precedence! {
                // 优先级最低：赋值运算（右结合）
                // 匹配模式：标识符 = 表达式
                x:(@) whitespace() "=" whitespace() y:@ {
                    if let Expr::Identifier(name) = x {
                        Expr::Assign { name, value: Box::new(y) }
                    } else {
                        // 实际工程中应返回 Result::Err，此处简化处理
                        panic!("Invalid assignment target")
                    }
                }
                --
                // 优先级中等：加减法（左结合）
                x:(@) whitespace() "+" whitespace() y:@ { Expr::BinaryOp { op: Op::Add, lhs: Box::new(x), rhs: Box::new(y) } }
                x:(@) whitespace() "-" whitespace() y:@ { Expr::BinaryOp { op: Op::Sub, lhs: Box::new(x), rhs: Box::new(y) } }
                --
                // 优先级最高：乘除法（左结合）
                x:(@) whitespace() "*" whitespace() y:@ { Expr::BinaryOp { op: Op::Mul, lhs: Box::new(x), rhs: Box::new(y) } }
                x:(@) whitespace() "/" whitespace() y:@ { Expr::BinaryOp { op: Op::Div, lhs: Box::new(x), rhs: Box::new(y) } }
                --
                // 原子项
                n:number() { Expr::Literal(n) }
                i:identifier() { Expr::Identifier(i) }
                // 括号表达式
                "(" whitespace() e:expression() whitespace() ")" { e }
            }

        rule number() -> i64
            = n:$("-"? ['0'..='9']+) {? n.parse().or(Err("number")) }

        rule identifier() -> String
            = s:$(['a'..='z' | 'A'..='Z' | '_']['a'..='z' | 'A'..='Z' | '0'..='9' | '_']*) { s.to_string() }
    }
}