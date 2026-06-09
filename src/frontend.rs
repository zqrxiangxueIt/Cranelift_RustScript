/// AST 节点 — 表达式
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal(String, Type),                   // 字面量 "42", "3.14" + 类型
    StringLiteral(String),                   // "hello" 字符串
    ComplexLiteral(f64, f64, Type),          // 1.5 + 2.5i (实部, 虚部, 类型)
    ArrayLiteral(Vec<Expr>, Type),           // [1, 2, 3] 固定数组
    DynamicArrayLiteral(Vec<Expr>, Type),    // array [1, 2, 3] 动态数组
    Identifier(String),                      // 变量名
    Assign(String, Box<Expr>),               // x = expr
    Eq(Box<Expr>, Box<Expr>),                // ==
    Ne(Box<Expr>, Box<Expr>),                // !=
    Lt(Box<Expr>, Box<Expr>),                // <
    Le(Box<Expr>, Box<Expr>),                // <=
    Gt(Box<Expr>, Box<Expr>),                // >
    Ge(Box<Expr>, Box<Expr>),                // >=
    Add(Box<Expr>, Box<Expr>),               // +
    Sub(Box<Expr>, Box<Expr>),               // -
    Mul(Box<Expr>, Box<Expr>),               // *
    Div(Box<Expr>, Box<Expr>),               // /
    IfElse(Box<Expr>, Vec<Expr>, Vec<Expr>), // if-else
    WhileLoop(Box<Expr>, Vec<Expr>),         // while 循环
    Call(String, Vec<Expr>),                 // 函数调用
    Index(Box<Expr>, Box<Expr>),             // arr[idx] 索引
    GlobalDataAddr(String),                  // &name 全局数据地址
    Cast(Box<Expr>, Type),                   // expr as Type
    Drop(String),                            // drop(var) 显式释放
    Block(Vec<Expr>),                        // 块作用域 { stmts }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    I8,
    I16,
    I32,
    I64,
    // I128: 当前实现不完整，字面量会被截断为 i64。
    // 完整的 i128 支持需要使用两个 i64 拼接实现。
    I128,
    F32,
    F64,
    String,
    Complex64,
    Complex128,
    Array(Box<Type>, usize), // Fixed size array for now
    DynamicArray(Box<Type>),
}

peg::parser!(pub grammar parser() for str {    //peg 是 Parsing Expression Grammars 的 Rust 实现的第三方 crate
    use super::{Expr, Type};
    //use — 把路径里的项引入到当前作用域
    //super — 模块路径里的"上一级"，
    //即在从父模块开始找 Expr（表达式枚举）和 Type（类型枚举），这样我们就可以在语法规则里直接使用它们了
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
    //s.into_iter().flatten().collect()把"嵌套的 Vec"拍平成一个单层的 Vec<Expr> 返回，因为statement()规则返回的是 Option<Expr>，所以会有一些 None 需要过滤掉，最终得到一个只包含 Some(Expr) 的 Vec<Expr>。
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
        / block_stmt()
        / "drop" _ "(" _ i:identifier() _ ")" { Expr::Drop(i) }
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

    /// 块作用域：{ stmts }
    /// PEG 有序选择天然消除歧义——if/while 以关键字开头，不会匹配独立的 {
    rule block_stmt() -> Expr
        = "{" _ "\n"
        body:statements() _ "}" _
        { Expr::Block(body) }

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
    //a:@ _ "[" _ idx:expression() _ "]"匹配 arr[0]、darr[i+1] 这种下标访问
    //这里调用的是完整的顶层 expression()，不是 binary_op()，所以索引里可以塞 if/while/赋值等任意表达式，比如 arr[if i > 0 { i } else { 0 }]
    //i:identifier() _ "(" args:((_ e:expression() _ {e}) ** ",") ")" 函数调用，匹配 foo(a, b, c)、puts("hello") 这种调用
    //** "," 允许0 个参数

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
    //array<T> 和 [T; N] 是 Toy 里仅有的两种"带参数类型"语法，分别构造 Type::DynamicArray(Box<Type>) 和 Type::Array(Box<Type>, usize)。t:type_name() 的递归让它们能任意嵌套，$(...) 让 len 拿到原始数字字符串供后续解析。语法直接照搬 Rust，只在 type_name() 内部生效，不会和数组字面量 [1, 2, 3] 冲突，因为分隔符（; vs ,）和元素语法（type_name vs expression）不同。

    //$ 符号 ：这是 PEG 的操作符，意思是“捕获匹配到的原始字符串”。如果不加 $ ，匹配成功了但你拿不到具体的文本内容
    //返回值 ：把捕获到的切片转成 String 返回
    rule identifier() -> String
        = quiet!{
            n:$(!(keyword() !['a'..='z' | 'A'..='Z' | '0'..='9' | '_']) ['a'..='z' | 'A'..='Z' | '_']['a'..='z' | 'A'..='Z' | '0'..='9' | '_']*)
            { n.to_owned() }
        }
        / expected!("identifier")
//
//用 { n.to_owned() } 把 &str 转成 String（函数签名要求返回 String）
//keyword()：尝试匹配任意一个关键字（fn / if / else / while / as / array / i8..i128 / f32 / f64 / string / complex64 / complex128）
// 负向字符类：!['a'..='z' | 'A'..='Z' | '0'..='9' | '_']
// 要求当前位置的字符不是字母/数字/下划线（也就是"非标识符字符"）
// 然后后面的['a'..='z' | 'A'..='Z' | '_'] 要求当前位置的字符必须是字母或下划线（也就是"标识符开头字符"）
// 最后 ['a'..='z' | 'A'..='Z' | '0'..='9' | '_']* 匹配标识符的剩余部分（可以是字母、数字或下划线，允许有0个）
//通过两层否定的负向预查 精确判断"当前位置是'关键字 + 非标识符字符'还是'真标识符'"

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

    // 负责解析 Toy 语法里所有源代码里直接写出来的常量值
    //顺序为：字符串字面量、复数字面量、动态数组字面量、固定数组字面量、浮点数字面量、整数字面量、全局数据地址
    //array 关键字是区分固定数组还是动态数组的，因为它们的语法不同（array [1, 2, 3] vs [1, 2, 3]），所以放在不同的规则里解析
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
