use crate::frontend::{self, Expr, Type as FrontendType, parser};
use cranelift::codegen::ir::BlockArg;
use cranelift::codegen::ir::InstBuilder;
use cranelift::prelude::*;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{DataDescription, Linkage, Module};
use std::collections::HashMap;
use std::slice;

/// The basic JIT class.
pub struct JIT {
    /// The function builder context, which is reused across multiple
    /// FunctionBuilder instances.
    builder_context: FunctionBuilderContext,

    /// The main Cranelift context, which holds the state for codegen. Cranelift
    /// separates this from `Module` to allow for parallel compilation, with a
    /// context per thread, though this isn't in the simple demo here.
    ctx: codegen::Context,

    /// The data description, which is to data objects what `ctx` is to functions.
    data_description: DataDescription,

    /// The module, with the jit backend, which manages the JIT'd
    /// functions.
    module: JITModule,
}

impl Default for JIT {
    fn default() -> Self {
        let mut flag_builder = settings::builder();
        flag_builder.set("use_colocated_libcalls", "false").unwrap();
        flag_builder.set("is_pic", "false").unwrap();
        let isa_builder = cranelift_native::builder().unwrap_or_else(|msg| {
            panic!("host machine is not supported: {}", msg);
        });
        let isa = isa_builder
            .finish(settings::Flags::new(flag_builder))
            .unwrap();
        let builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());

        let module = JITModule::new(builder);
        Self {
            builder_context: FunctionBuilderContext::new(),
            ctx: module.make_context(),
            data_description: DataDescription::new(),
            module,
        }
    }
}

impl JIT {
    /// Compile a string in the toy language into machine code.
    pub fn compile(&mut self, input: &str) -> Result<*const u8, String> {
        // 首先，解析字符串，生成AST节点
        let (name, params, the_return, stmts) =
            parser::function(input).map_err(|e| e.to_string())?;      

        // 接着，将AST节点转换为Cranelift IR
        self.translate(name.clone(), params, the_return, stmts)?;

        // 最后，声明函数并定义它
        // 导出函数，使外部代码可以调用它
        let id = self
            .module
            .declare_function(&name, Linkage::Export, &self.ctx.func.signature)
            .map_err(|e| e.to_string())?;

        // 定义函数，将Cranelift IR转换为机器码
        self.module
            .define_function(id, &mut self.ctx)
            .map_err(|e| e.to_string())?;

        // 编译完成后，清除上下文状态
        self.module.clear_context(&mut self.ctx);

        // 最终ize定义的函数
        self.module.finalize_definitions().unwrap();

        // 现在可以检索指向机器码的指针
        let code = self.module.get_finalized_function(id);

        Ok(code)
    }

    /// 创建一个零初始化的数据段
    pub fn create_data(&mut self, name: &str, contents: Vec<u8>) -> Result<&[u8], String> {
        self.data_description.define(contents.into_boxed_slice());
        let id = self
            .module
            .declare_data(name, Linkage::Export, true, false)
            .map_err(|e| e.to_string())?;

        self.module
            .define_data(id, &self.data_description)
            .map_err(|e| e.to_string())?;
        self.data_description.clear();
        self.module.finalize_definitions().unwrap();
        let buffer = self.module.get_finalized_data(id);
        Ok(unsafe { slice::from_raw_parts(buffer.0, buffer.1) })
    }

    // 将toy语言的AST节点转换为Cranelift IR
    /// 参数:
    /// - name: 函数名
    /// - params: 参数列表，每个参数包含名称和类型
    /// - the_return: 返回变量名及其类型
    /// - stmts: 函数体语句列表
    fn translate(
        &mut self,
        name: String,
        params: Vec<(String, FrontendType)>,
        the_return: (String, FrontendType),
        stmts: Vec<Expr>,
    ) -> Result<(), String> {
        // 将参数类型添加到函数签名中
        for (_, ty) in &params {
            self.ctx.func.signature.params.push(AbiParam::new(to_cranelift_type(*ty)));
        }

        // 将返回类型添加到函数签名中
        self.ctx.func.signature.returns.push(AbiParam::new(to_cranelift_type(the_return.1)));

        // 创建函数构建器并设置入口块
        let mut builder = FunctionBuilder::new(&mut self.ctx.func, &mut self.builder_context);
        let entry_block = builder.create_block();

        // 将函数参数绑定到入口块的参数，并切换到入口块
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        // 声明所有变量（参数、返回变量及隐式变量）
        let variables = declare_variables(
            &mut builder,
            &params,
            &stmts,
            entry_block,
            &the_return,
        );

        // 创建表达式翻译器
        let mut trans = FunctionTranslator {
            builder,
            variables,
            module: &mut self.module,
            current_func_name: name,
            current_func_ret_type: to_cranelift_type(the_return.1),
        };

        // 逐条翻译函数体语句
        for expr in stmts {
            trans.translate_expr(expr);
        }
        
        // 读取返回变量的值并生成 return 指令
        let return_variable = trans.variables.get(&the_return.0).expect("return variable not defined");
        let return_value = trans.builder.use_var(*return_variable);
        trans.builder.ins().return_(&[return_value]);
        
        // 完成函数构建
        trans.builder.finalize();
        Ok(())
    }
}

fn to_cranelift_type(t: FrontendType) -> types::Type {
    match t {
        FrontendType::I8 => types::I8,
        FrontendType::I16 => types::I16,
        FrontendType::I32 => types::I32,
        FrontendType::I64 => types::I64,
        FrontendType::I128 => types::I128,
        FrontendType::F32 => types::F32,
        FrontendType::F64 => types::F64,
    }
}

struct FunctionTranslator<'a> {
    builder: FunctionBuilder<'a>,    // 函数构建器
    variables: HashMap<String, Variable>,    // 变量映射表
    module: &'a mut JITModule,              // JIT模块引用
    current_func_name: String,              // 当前函数名
    current_func_ret_type: types::Type,    // 当前函数返回类型
}

impl<'a> FunctionTranslator<'a> {
    fn translate_expr(&mut self, expr: Expr) -> Value {         //梯度下降翻译
        match expr {
            Expr::Literal(val, ty) => {      //// 翻译字面量
                let cl_ty: types::Type = to_cranelift_type(ty);
                match ty {
                    FrontendType::F32 => self.builder.ins().f32const(val.parse::<f32>().unwrap()),
                    FrontendType::F64 => self.builder.ins().f64const(val.parse::<f64>().unwrap()),
                    _ => {
                         let int_val = val.parse::<i128>().unwrap();
                         if cl_ty == types::I128 {
                             // Treat i128 literals as i64 for now as iconst supports Imm64. 
                             // Real i128 support would require constructing the value from two i64s.
                             InstBuilder::iconst(self.builder.ins(), cl_ty, int_val as i64) 
                         } else {
                             InstBuilder::iconst(self.builder.ins(), cl_ty, int_val as i64)
                         }
                    }
                }
            }

            Expr::Add(lhs, rhs) => self.translate_binary_op(*lhs, *rhs, |b, l, r| {
                 let ty = b.func.dfg.value_type(l);
                 if ty.is_float() { b.ins().fadd(l, r) } else { b.ins().iadd(l, r) }
            }),
            Expr::Sub(lhs, rhs) => self.translate_binary_op(*lhs, *rhs, |b, l, r| {
                 let ty = b.func.dfg.value_type(l);
                 if ty.is_float() { b.ins().fsub(l, r) } else { b.ins().isub(l, r) }
            }),
            Expr::Mul(lhs, rhs) => self.translate_binary_op(*lhs, *rhs, |b, l, r| {
                 let ty = b.func.dfg.value_type(l);
                 if ty.is_float() { b.ins().fmul(l, r) } else { b.ins().imul(l, r) }
            }),
            Expr::Div(lhs, rhs) => self.translate_binary_op(*lhs, *rhs, |b, l, r| {
                 let ty = b.func.dfg.value_type(l);
                 if ty.is_float() { b.ins().fdiv(l, r) } else { b.ins().udiv(l, r) }
            }),

            Expr::Eq(lhs, rhs) => self.translate_cmp(*lhs, *rhs, IntCC::Equal, FloatCC::Equal),
            Expr::Ne(lhs, rhs) => self.translate_cmp(*lhs, *rhs, IntCC::NotEqual, FloatCC::NotEqual),
            Expr::Lt(lhs, rhs) => self.translate_cmp(*lhs, *rhs, IntCC::SignedLessThan, FloatCC::LessThan),
            Expr::Le(lhs, rhs) => self.translate_cmp(*lhs, *rhs, IntCC::SignedLessThanOrEqual, FloatCC::LessThanOrEqual),
            Expr::Gt(lhs, rhs) => self.translate_cmp(*lhs, *rhs, IntCC::SignedGreaterThan, FloatCC::GreaterThan),
            Expr::Ge(lhs, rhs) => self.translate_cmp(*lhs, *rhs, IntCC::SignedGreaterThanOrEqual, FloatCC::GreaterThanOrEqual),

            Expr::Call(name, args) => self.translate_call(name, args),
            Expr::GlobalDataAddr(name) => self.translate_global_data_addr(name),
            Expr::Identifier(name) => {
                let variable = self.variables.get(&name).expect("variable not defined");
                self.builder.use_var(*variable)
            }
            Expr::Assign(name, expr) => self.translate_assign(name, *expr),
            Expr::IfElse(condition, then_body, else_body) => {
                self.translate_if_else(*condition, then_body, else_body)
            }
            Expr::WhileLoop(condition, loop_body) => {
                self.translate_while_loop(*condition, loop_body)
            }
            Expr::Cast(expr, target_ty) => {
                let val = self.translate_expr(*expr);
                self.translate_cast(val, to_cranelift_type(target_ty))
            }
        }
    }
    
    fn translate_binary_op<F>(&mut self, lhs: Expr, rhs: Expr, op: F) -> Value 
    where F: Fn(&mut FunctionBuilder, Value, Value) -> Value {
        let l_val = self.translate_expr(lhs);    /// 先把左边的表达式翻译完，拿到结果线头
        let r_val = self.translate_expr(rhs);    /// 再把右边的表达式翻译完，拿到结果线头
        let (l_promoted, r_promoted) = self.promote_operands(l_val, r_val);
        //如果左边是 i32，右边是 i64，要把左边“拉长”成 i64
        op(&mut self.builder, l_promoted, r_promoted)  //// 生成真正的加法指令
    }   
    
    fn promote_operands(&mut self, lhs: Value, rhs: Value) -> (Value, Value) {
        let l_ty = self.builder.func.dfg.value_type(lhs);
        let r_ty = self.builder.func.dfg.value_type(rhs);
        
        if l_ty == r_ty {
            return (lhs, rhs);
        }
        
        // Implicit promotion: int -> wider int, float -> wider float.
        // No implicit int <-> float.
        
        if l_ty.is_int() && r_ty.is_int() {
             if l_ty.bits() < r_ty.bits() {
                 let l_new_s = self.builder.ins().sextend(r_ty, lhs);
                 return (l_new_s, rhs);
             } else {
                 let r_new_s = self.builder.ins().sextend(l_ty, rhs);
                 return (lhs, r_new_s);
             }
        }
        
        if l_ty.is_float() && r_ty.is_float() {
             if l_ty.bits() < r_ty.bits() {
                 let l_new = self.builder.ins().fpromote(r_ty, lhs);
                 return (l_new, rhs);
             } else {
                 let r_new = self.builder.ins().fpromote(l_ty, rhs);
                 return (lhs, r_new);
             }
        }
        
        panic!("Incompatible types in operation: {:?} vs {:?}", l_ty, r_ty);
    }

    /// 翻译类型转换
    fn translate_cast(&mut self, val: Value, target_ty: types::Type) -> Value {
        let src_ty = self.builder.func.dfg.value_type(val);
        if src_ty == target_ty { return val; }
        
        if src_ty.is_int() && target_ty.is_int() {
            if src_ty.bits() < target_ty.bits() {
                return self.builder.ins().sextend(target_ty, val);
            } else {
                return self.builder.ins().ireduce(target_ty, val);
            }
        }               //整数转整数 (Int -> Int)
        
        if src_ty.is_float() && target_ty.is_float() {   //浮点转浮点 (Float -> Float)
             if src_ty.bits() < target_ty.bits() {
                 return self.builder.ins().fpromote(target_ty, val);
             } else {
                 return self.builder.ins().fdemote(target_ty, val);
             }
        }
        
        if src_ty.is_int() && target_ty.is_float() {        //整数转浮点 (Int -> Float)
            return self.builder.ins().fcvt_from_sint(target_ty, val);
        }
        
        if src_ty.is_float() && target_ty.is_int() {
            return self.builder.ins().fcvt_to_sint(target_ty, val);
        }
        
        panic!("Unsupported cast from {:?} to {:?}", src_ty, target_ty);
    }

    ///比较操作
    fn translate_cmp(&mut self, lhs: Expr, rhs: Expr, int_cc: IntCC, float_cc: FloatCC) -> Value {
        let l_val = self.translate_expr(lhs);
        let r_val = self.translate_expr(rhs);
        let (l, r) = self.promote_operands(l_val, r_val);
        let ty = self.builder.func.dfg.value_type(l);
        
        let bool_res = if ty.is_float() {
            self.builder.ins().fcmp(float_cc, l, r)
        } else {
            self.builder.ins().icmp(int_cc, l, r)
        };
        // Use select instead of bint if bint is missing
        let one = InstBuilder::iconst(self.builder.ins(), types::I64, 1);
        let zero = InstBuilder::iconst(self.builder.ins(), types::I64, 0);
        self.builder.ins().select(bool_res, one, zero)
    }
    ///变量赋值
    fn translate_assign(&mut self, name: String, expr: Expr) -> Value {
        let new_value = self.translate_expr(expr);
        let variable = self.variables.get(&name).unwrap();
        self.builder.def_var(*variable, new_value);
        new_value
    }
        
    /// if-else 语句
    fn translate_if_else(
        &mut self,
        condition: Expr,
        then_body: Vec<Expr>,
        else_body: Vec<Expr>,
    ) -> Value {
        let condition_value = self.translate_expr(condition);

        let then_block = self.builder.create_block();
        let else_block = self.builder.create_block();
        let merge_block = self.builder.create_block();

        self.builder
            .ins()
            .brif(condition_value, then_block, &[], else_block, &[]);

        self.builder.switch_to_block(then_block);
        self.builder.seal_block(then_block);
        let mut then_return = InstBuilder::iconst(self.builder.ins(), types::I64, 0); // Default
        for expr in then_body {
            then_return = self.translate_expr(expr);
        }
        let then_ty = self.builder.func.dfg.value_type(then_return);
        self.builder.append_block_param(merge_block, then_ty);
        self.builder.ins().jump(merge_block, &[BlockArg::Value(then_return)]);

        self.builder.switch_to_block(else_block);
        self.builder.seal_block(else_block);
        let mut else_return = InstBuilder::iconst(self.builder.ins(), types::I64, 0);
        for expr in else_body {
            else_return = self.translate_expr(expr);
        }
        
        // Explicitly cast else result to match then result type (simple unification)
        let else_return_cast = if then_ty != self.builder.func.dfg.value_type(else_return) {
             // For simplicity, we just use else_return and hope for the best or rely on validation error.
             // Implementing proper cast here requires access to self.translate_cast which takes &mut self.
             // We can call it!
             self.translate_cast(else_return, then_ty)
        } else {
             else_return
        };
        self.builder.ins().jump(merge_block, &[BlockArg::Value(else_return_cast)]);

        self.builder.switch_to_block(merge_block);
        self.builder.seal_block(merge_block);

        self.builder.block_params(merge_block)[0]
    }

    /// while 循环语句
    fn translate_while_loop(&mut self, condition: Expr, loop_body: Vec<Expr>) -> Value {
        let header_block = self.builder.create_block();
        let body_block = self.builder.create_block();
        let exit_block = self.builder.create_block();

        self.builder.ins().jump(header_block, &[]);
        self.builder.switch_to_block(header_block);

        let condition_value = self.translate_expr(condition);
        self.builder
            .ins()
            .brif(condition_value, body_block, &[], exit_block, &[]);

        self.builder.switch_to_block(body_block);
        self.builder.seal_block(body_block);

        for expr in loop_body {
            self.translate_expr(expr);
        }
        self.builder.ins().jump(header_block, &[]);

        self.builder.switch_to_block(exit_block);
        self.builder.seal_block(header_block);
        self.builder.seal_block(exit_block);

        InstBuilder::iconst(self.builder.ins(), types::I64, 0)
    }

    /// 函数调用
    fn translate_call(&mut self, name: String, args: Vec<Expr>) -> Value {
        let mut sig = self.module.make_signature();
        
        let mut arg_values = Vec::new();
        for arg in args {
            let val = self.translate_expr(arg);
            arg_values.push(val);
            sig.params.push(AbiParam::new(self.builder.func.dfg.value_type(val)));
        }

        // Return type?
        if name == self.current_func_name {
            sig.returns.push(AbiParam::new(self.current_func_ret_type));
        } else {
             // Assume I64 for unknown functions
             sig.returns.push(AbiParam::new(types::I64));
        }

        let callee = self
            .module
            .declare_function(&name, Linkage::Import, &sig)
            .expect("problem declaring function");
        let local_callee = self.module.declare_func_in_func(callee, self.builder.func);

        let call = self.builder.ins().call(local_callee, &arg_values);
        self.builder.inst_results(call)[0]
    }

    /// 获取全局数据的内存地址
    fn translate_global_data_addr(&mut self, name: String) -> Value {
        let sym = self
            .module
            .declare_data(&name, Linkage::Export, true, false)
            .expect("problem declaring data object");
        let local_id = self.module.declare_data_in_func(sym, self.builder.func);

        let pointer = self.module.target_config().pointer_type();
        self.builder.ins().symbol_value(pointer, local_id)
    }
}

/// 在 JIT 编译开始前 扫描并声明所有变量
fn declare_variables(
    builder: &mut FunctionBuilder,
    params: &[(String, FrontendType)],
    stmts: &[Expr],
    entry_block: Block,
    return_info: &(String, FrontendType),
) -> HashMap<String, Variable> {
    let mut variables = HashMap::new();
    
    // - 注册 ：为每个函数参数创建一个 Cranelift 变量（ declare_var ）。
    // - 绑定 ：把函数的 入口参数值 （ block_params ）赋给这个变量（ def_var ）。
    for (i, (name, ty)) in params.iter().enumerate() {
        let val = builder.block_params(entry_block)[i];
        let var = builder.declare_var(to_cranelift_type(*ty));
        variables.insert(name.clone(), var);
        builder.def_var(var, val);
    }
    
    /// 声明返回值变量
    let (ret_name, ret_ty) = return_info;
    if !variables.contains_key(ret_name) {
        let cl_ty = to_cranelift_type(*ret_ty);
        let var = builder.declare_var(cl_ty);
        variables.insert(ret_name.clone(), var);
        
        // Initialize return var to 0 or equivalent
        let zero = match ret_ty {
            FrontendType::F32 => builder.ins().f32const(0.0),
            FrontendType::F64 => builder.ins().f64const(0.0),
            _ => InstBuilder::iconst(builder.ins(), cl_ty, 0),
        };
        builder.def_var(var, zero);
    }
    
    /// 扫描语句中的隐式变量
    for expr in stmts {
        declare_variables_in_stmt(builder, &mut variables, expr);
    }

    variables
}

/// 递归扫描表达式中的变量声明
fn declare_variables_in_stmt(
    builder: &mut FunctionBuilder,
    variables: &mut HashMap<String, Variable>,
    expr: &Expr,
) {
    match *expr {
        Expr::Assign(ref name, ref val_expr) => {
            if !variables.contains_key(name) {
                // Infer type
                let ty = infer_type(val_expr, variables); // We need inferred type!
                let var = builder.declare_var(to_cranelift_type(ty));
                variables.insert(name.clone(), var);
            }
        }
        Expr::IfElse(ref _condition, ref then_body, ref else_body) => {
            for stmt in then_body {
                declare_variables_in_stmt(builder, variables, stmt);
            }
            for stmt in else_body {
                declare_variables_in_stmt(builder, variables, stmt);
            }
        }
        Expr::WhileLoop(ref _condition, ref loop_body) => {
            for stmt in loop_body {
                declare_variables_in_stmt(builder, variables, stmt);
            }
        }
        _ => (),
    }
}

/// 递归推断表达式的类型
fn infer_type(expr: &Expr, variables: &HashMap<String, Variable>) -> FrontendType {
    // Simple inference. 
    // Limitation: if we use a variable declared later, we fail.
    // Also we don't know the type of `variables` entries here (Variable doesn't store type).
    // So this is imperfect. Ideally we need a symbol table with types.
    // For this demo, let's assume default I64 if unknown or recurse.
    match expr {
        Expr::Literal(_, ty) => *ty,
        Expr::Cast(_, ty) => *ty,
        Expr::Add(lhs, _) => infer_type(lhs, variables), // Assume lhs type dominates
        Expr::Sub(lhs, _) => infer_type(lhs, variables),
        Expr::Mul(lhs, _) => infer_type(lhs, variables),
        Expr::Div(lhs, _) => infer_type(lhs, variables),
        Expr::Identifier(_) => FrontendType::I64, // Fallback, we can't easily know without tracking types in variables map
        Expr::Call(_, _) => FrontendType::I64, // Fallback
        _ => FrontendType::I64,
    }
}
