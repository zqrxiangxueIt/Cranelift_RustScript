/// The AST node for expressions.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal(String, Type), // Value, Type (Integer or Float)
    StringLiteral(String),
    ComplexLiteral(f64, f64, Type), // Real, Imag, Type (Complex64 or Complex128)
    ArrayLiteral(Vec<Expr>, Type),
    DynamicArrayLiteral(Vec<Expr>, Type),
    Identifier(String),
    Assign(String, Box<Expr>),
    Eq(Box<Expr>, Box<Expr>),
    Ne(Box<Expr>, Box<Expr>),
    Lt(Box<Expr>, Box<Expr>),
    Le(Box<Expr>, Box<Expr>),
    Gt(Box<Expr>, Box<Expr>),
    Ge(Box<Expr>, Box<Expr>),
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Div(Box<Expr>, Box<Expr>),
    IfElse(Box<Expr>, Vec<Expr>, Vec<Expr>),
    WhileLoop(Box<Expr>, Vec<Expr>),
    Call(String, Vec<Expr>),
    Index(Box<Expr>, Box<Expr>), // Array indexing
    GlobalDataAddr(String),
    Cast(Box<Expr>, Type),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    I8,
    I16,
    I32,
    I64,
    I128,
    F32,
    F64,
    String,
    Complex64,
    Complex128,
    Array(Box<Type>, usize), // Fixed size array for now
    DynamicArray(Box<Type>),
}

peg::parser!(pub grammar parser() for str {
    use super::{Expr, Type};
    /// rule是peg宏的关键字，表示定义一个语法规则
    /// function()规则用于解析函数定义，返回一个元组，包含函数名、参数列表、返回值类型和函数体语句列表
    /// // c = a + b 的表示：
            //Expr::Assign(
            //"c".to_string(),                    // 赋值的目标变量名
            //Box::new(                           // 赋值的内容（右值）
                //Expr::Add(                      // 加法表达式 a + b
                //    Box::new(Expr::Identifier("a".to_string())), // 左操作数 a
                //    Box::new(Expr::Identifier("b".to_string()))  // 右操作数 b
                    //)
                //)
            //)
    pub rule function() -> (String, Vec<(String, Type)>, (String, Type), Vec<Expr>)
        //允许在函数定义的最开始出现任意数量（ * ）的空格、制表符或换行符；要求 接下来必须紧跟字符串 fn；
        // _ ：这是一个在别处定义的规则（通常代表任意空白字符），表示允许 fn 和名字之间有空格调用 
        //identifier() 规则去解析一个标识符（比如 add ），把解析出来的结果（一个字符串）赋值给变量 name
        = [' ' | '\t' | '\n']* "fn" _ name:identifier() _ 
        //"(" ... ")" ：要求必须有一对圆括号包裹。
        // params:(...) ：把括号里解析出来的内容赋值给 params 变量。
        // (...) ** ","：这是一个 PEG 的特殊语法，意思是 “被逗号分隔的列表” 
        //内部结构 (_ i:identifier() _ ":" _ t:type_name() _ {(i, t)}) ：
            //- i:identifier() ：解析参数名，存入 i 。
            //- ":" ：中间必须有个冒号。
            //- t:type_name() ：解析类型名，存入 t 。
            //- {(i, t)} ：这是 Rust 代码块。对于每一个参数，把它打包成一个 Rust 元组 (参数名, 类型) 返回 
        //即这一行会解析出像 (a: i32, b: i64) 这样的结构，并生成一个 Vec<(String, Type)>     
        "(" params:((_ i:identifier() _ ":" _ t:type_name() _ {(i, t)}) ** ",") ")" _
        "->" _
        // 部逻辑和参数列表完全一样：解析 名字: 类型 （例如 r: i64 ），并打包成 (String, Type)
        //注意：这里没有 ** "," ，说明你的语言目前只支持 单个返回值
        "(" ret:(_ i:identifier() _ ":" _ t:type_name() _ {(i, t)}) ")" _
        "{" _ "\n"
        //- 调用 statements() 规则。这个规则会解析花括号里的一系列语句（比如 a = 1; b = 2; ）。
        //- 结果存入 stmts 变量（类型是 Vec<Expr> ）。
        //- 最后返回一个元组 (name, params, ret, stmts) ，包含函数名、参数列表、返回值类型和语句列表。
        stmts:statements()
        _ "}" _ "\n" _
        { (name, params, ret, stmts) }

    //一个“语句块”是由 0个或多个 （ * ）“单条语句”组成的序列
    //statement()*会不断调用 statement() 规则，直到无法匹配为止
    //匹配到的所有结果会自动收集成一个 Vec （向量/列表）
    rule statements() -> Vec<Expr>
        = s:(statement()*) { s.into_iter().flatten().collect() }
    //statement() ：单条语句的定义
    //e:expression() _ ：调用更底层的 expression() 规则来解析实际的逻辑（比如 a + b 或 c = 1 ）
        //前后允许有空白字符 _
        //结果存入变量 e
    //"\n"明确规定：每一条语句后面 必须 跟一个换行符
    //{ e } ：解析成功后，把表达式 e 返回
    rule statement() -> Option<Expr>
        = _ e:expression() _ "\n" { Some(e) }
        / _ "\n" { None }

    //expression() ：表达式的定义
    //if_else() / while_loop() / assignment() / binary_op() ：
        //- 分别调用对应的规则来解析不同类型的表达式（比如 if 语句、while 循环、赋值语句、二元操作符）
        //- 每个规则都有自己的语法和优先级，确保解析顺序正确，用/表示匹配优先级（这里先匹配if-else）
    rule expression() -> Expr
        = if_else()
        / while_loop()
        / assignment()          //表示赋值语句，例如 a = 1
        / binary_op()           //表示二元操作符，例如 a + b 或 a * b

    rule if_else() -> Expr
        = "if" _ e:expression() _ "{" _ "\n"
        then_body:statements() _ "}" _ "else" _ "{" _ "\n"
        else_body:statements() _ "}"
        { Expr::IfElse(Box::new(e), then_body, else_body) }

    rule while_loop() -> Expr
        = "while" _ e:expression() _ "{" _ "\n"
        loop_body:statements() _ "}"
        { Expr::WhileLoop(Box::new(e), loop_body) }

    ///变量赋值语法，identifier()明确规定左边 必须是一个标识符。匹配到的变量名（字符串）存入变量 i
    /// e:expression()匹配赋值号右边的部分（右值），右边可以是 任意表达式 （数字、运算、函数调用、甚至另一个赋值）
    rule assignment() -> Expr
        = i:identifier() _ "=" _ e:expression() {Expr::Assign(i, Box::new(e))}

    ///二元操作符语法，precedence!{} ：定义操作符的优先级。
    ///- 每个操作符都有一个优先级，数字越大优先级越高。
    ///- 每个操作符的定义格式是 a:@ _ "操作符" _ b:(@) { 表达式 }
    ///    - a:@ ：左边的操作数，用 @ 表示“捕获”（匹配到的内容会被暂存起来）
    ///    - _ "操作符" _ ：中间的操作符，这里是 + 或 -
    ///    - b:(@) ：右边的操作数，也用 @ 表示“捕获”
    ///    - { 表达式 } ：匹配成功后，执行的 Rust 代码，这里是创建一个 Add 或 Sub 表达式            
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
        --
        a:@ _ "as" _ t:type_name() { Expr::Cast(Box::new(a), t) }
        --
        a:@ _ "[" _ idx:expression() _ "]" { Expr::Index(Box::new(a), Box::new(idx)) }
        i:identifier() _ "(" args:((_ e:expression() _ {e}) ** ",") ")" { Expr::Call(i, args) }
        i:identifier() { Expr::Identifier(i) }
        l:literal() { l }
        "(" _ e:expression() _ ")" { e }
    }
    ///解析过程 ( a + b * c ) ：
    ///- 解析器首先尝试匹配最外层的低优先级规则（加法层）。
    ///- 它识别出 + 号。 + 号左边的 a 会“向下”递归去匹配更高优先级的规则，最终匹配到一个标识符 a 。
    ///- + 号右边的 b * c ，由于 * 定义在更下层的规则中（Level 3），所以解析器会优先将 b 和 c 按照乘法规则组合在一起。
    ///- 最终结果就是： Expr::Add(a, Expr::Mul(b, c))


    rule type_name() -> Type
        = "i8" { Type::I8 }
        / "i16" { Type::I16 }
        / "i32" { Type::I32 }
        / "i64" { Type::I64 }
        / "i128" { Type::I128 }
        / "f32" { Type::F32 }
        / "f64" { Type::F64 }
        / "string" { Type::String }
        / "complex64" { Type::Complex64 }
        / "complex128" { Type::Complex128 }
        / "array" _ "<" _ t:type_name() _ ">" { Type::DynamicArray(Box::new(t)) }
        / "[" _ t:type_name() _ ";" _ len:$(['0'..='9']+) _ "]" { 
            Type::Array(Box::new(t), len.parse().unwrap()) 
        }

    //$ 符号 ：这是 PEG 的操作符，意思是“捕获匹配到的原始字符串”。如果不加 $ ，匹配成功了但你拿不到具体的文本内容
    //返回值 ：把捕获到的切片转成 String 返回
    rule identifier() -> String
        = quiet!{ 
            n:$(!(keyword() !['a'..='z' | 'A'..='Z' | '0'..='9' | '_']) ['a'..='z' | 'A'..='Z' | '_']['a'..='z' | 'A'..='Z' | '0'..='9' | '_']*) 
            { n.to_owned() } 
        }
        / expected!("identifier")

    rule keyword()
        = "fn" / "if" / "else" / "while" / "as" / "array" / "i8" / "i16" / "i32" / "i64" / "i128" / "f32" / "f64" / "string" / "complex64" / "complex128"

    rule literal() -> Expr
        = s:string_literal() { Expr::StringLiteral(s) }
        / c:complex_literal() { c }
        / a:dynamic_array_literal() { a }
        / a:array_literal() { a }
        / n:$(['0'..='9']+ "." ['0'..='9']+) { Expr::Literal(n.to_owned(), Type::F64) }
        / n:$(['0'..='9']+) { Expr::Literal(n.to_owned(), Type::I64) }
        / "&" i:identifier() { Expr::GlobalDataAddr(i) }

    rule array_literal() -> Expr
        = "[" _ elems:((_ e:expression() _ {e}) ** ",") _ "]" {
            Expr::ArrayLiteral(elems, Type::I64) // Placeholder type, inferred in JIT
        }

    rule dynamic_array_literal() -> Expr
        = "array" _ "[" _ elems:((_ e:expression() _ {e}) ** ",") _ "]" {
            Expr::DynamicArrayLiteral(elems, Type::I64) // Placeholder type, inferred in JIT
        }

    rule string_literal() -> String
        = "\"" s:double_quoted_character()* "\"" { s.into_iter().collect() }

    rule double_quoted_character() -> char
        = !("\"" / "\\") c:any_char() { c }
        / "\\" esc:escape_sequence() { esc }

    rule escape_sequence() -> char
        = "\"" { '"' }
        / "\\" { '\\' }
        / "n" { '\n' }
        / "t" { '\t' }
        / "r" { '\r' }

    rule any_char() -> char
        = c:['\x00'..='\x7f'] { c } // ASCII only for simplicity, or use utf8


    rule complex_literal() -> Expr
        = r:$(['0'..='9']+ "." ['0'..='9']+) _ "+" _ i:$(['0'..='9']+ "." ['0'..='9']+) "i" {
            Expr::ComplexLiteral(r.parse().unwrap(), i.parse().unwrap(), Type::Complex128)
        }
        / i:$(['0'..='9']+ "." ['0'..='9']+) "i" {
            Expr::ComplexLiteral(0.0, i.parse().unwrap(), Type::Complex128)
        }

    rule _() =  quiet!{[' ' | '\t']*} 
});
