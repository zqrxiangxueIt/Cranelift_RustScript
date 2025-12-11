/// The AST node for expressions.
pub enum Expr {
    Literal(String),       // 字面量，例如数字 "123"
    Identifier(String),    // 标识符，即变量名，例如 "x"
    Assign(String, Box<Expr>),     // 赋值语句，例如 "x = 5"
    Eq(Box<Expr>, Box<Expr>),      // 等于 (Equal) ==
    Ne(Box<Expr>, Box<Expr>),       // 不等于 (Not Equal) !=
    Lt(Box<Expr>, Box<Expr>),       // 小于 (Less Than) <
    Le(Box<Expr>, Box<Expr>),       // 小于等于 (Less Equal) <=
    Gt(Box<Expr>, Box<Expr>),       // 大于 (Greater Than) >
    Ge(Box<Expr>, Box<Expr>),       // 大于等于 (Greater Equal) >=
    Add(Box<Expr>, Box<Expr>),      // 加法 +
    Sub(Box<Expr>, Box<Expr>),      // 减法 -
    Mul(Box<Expr>, Box<Expr>),      // 乘法 *
    Div(Box<Expr>, Box<Expr>),      // 除法 /
    Rem(Box<Expr>, Box<Expr>),
    IfElse(Box<Expr>, Vec<Expr>, Vec<Expr>),        // If-Else 结构：条件，Then块语句列表，Else块语句列表
    WhileLoop(Box<Expr>, Vec<Expr>),            // While 循环：条件，循环体语句列表
    Call(String, Vec<Expr>),                // 调用函数：函数名，参数列表
    GlobalDataAddr(String),                 // 获取全局数据地址 (类似C语言的取地址 &)
}

//Box<Expr>必须有确定的大小,使用 Box（智能指针）将数据存储在堆上，指针大小是固定的
//Vec<Expr>: 用于存储语句列表（代码块）或参数列表。

peg::parser!(pub grammar parser() for str {
    pub rule function() -> (String, Vec<String>, String, Vec<Expr>)
        = [' ' | '\t' | '\n']* "fn" _ name:identifier() _
        "(" params:((_ i:identifier() _ {i}) ** ",") ")" _
        "->" _
        "(" returns:(_ i:identifier() _ {i}) ")" _
        "{" _ "\n"
        stmts:statements()
        _ "}" _ "\n" _
        { (name, params, returns, stmts) }

    rule statements() -> Vec<Expr>
        = s:(statement()*) { s }

    rule statement() -> Expr
        = _ e:expression() _ "\n" { e }

    rule expression() -> Expr
        = if_else()
        / while_loop()
        / assignment()
        / binary_op()

    rule if_else() -> Expr
        = "if" _ e:expression() _ "{" _ "\n"
        then_body:statements() _ "}" _ "else" _ "{" _ "\n"
        else_body:statements() _ "}"
        { Expr::IfElse(Box::new(e), then_body, else_body) }

    rule while_loop() -> Expr
        = "while" _ e:expression() _ "{" _ "\n"
        loop_body:statements() _ "}"
        { Expr::WhileLoop(Box::new(e), loop_body) }

    rule assignment() -> Expr
        = i:identifier() _ "=" _ e:expression() {Expr::Assign(i, Box::new(e))}

    rule binary_op() -> Expr = precedence!{
        a:@ _ "==" _ b:(@) { Expr::Eq(Box::new(a), Box::new(b)) }
        a:@ _ "!=" _ b:(@) { Expr::Ne(Box::new(a), Box::new(b)) }
        a:@ _ "<"  _ b:(@) { Expr::Lt(Box::new(a), Box::new(b)) }
        a:@ _ "<=" _ b:(@) { Expr::Le(Box::new(a), Box::new(b)) }
        a:@ _ ">"  _ b:(@) { Expr::Gt(Box::new(a), Box::new(b)) }
        a:@ _ ">=" _ b:(@) { Expr::Ge(Box::new(a), Box::new(b)) }
        --
        a:@ _ "+" _ b:(@) { Expr::Add(Box::new(a), Box::new(b)) }
        a:@ _ "-" _ b:(@) { Expr::Sub(Box::new(a), Box::new(b)) }
        --
        a:@ _ "*" _ b:(@) { Expr::Mul(Box::new(a), Box::new(b)) }
        a:@ _ "/" _ b:(@) { Expr::Div(Box::new(a), Box::new(b)) }
        a:@ _ "%" _ b:(@) { Expr::Rem(Box::new(a), Box::new(b)) }
        --
        i:identifier() _ "(" args:((_ e:expression() _ {e}) ** ",") ")" { Expr::Call(i, args) }
        i:identifier() { Expr::Identifier(i) }
        l:literal() { l }
    }

    rule identifier() -> String
        = quiet!{ n:$(['a'..='z' | 'A'..='Z' | '_']['a'..='z' | 'A'..='Z' | '0'..='9' | '_']*) { n.to_owned() } }
        / expected!("identifier")

    rule literal() -> Expr
        = n:$(['0'..='9']+) { Expr::Literal(n.to_owned()) }
        / "&" i:identifier() { Expr::GlobalDataAddr(i) }

    rule _() =  quiet!{[' ' | '\t']*}
});
