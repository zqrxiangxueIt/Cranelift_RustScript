use crate::frontend::{Expr, Type as FrontendType, parser};
use crate::type_checker::{self, TypeChecker};
use crate::runtime;
use cranelift::codegen::ir::BlockArg;
use cranelift::codegen::ir::InstBuilder;
use cranelift::codegen::ir::{StackSlotData, StackSlotKind};
use cranelift::prelude::*;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{DataDescription, Linkage, Module};
use std::collections::HashMap;
use std::slice;

/// 基础 JIT 类。
pub struct JIT {
    /// 函数构建器上下文，在多个
    /// FunctionBuilder 实例之间重用。
    builder_context: FunctionBuilderContext,

    /// 主要的 Cranelift 上下文，保存代码生成的状态。Cranelift
    /// 将其与 `Module` 分离以允许并行编译，
    /// 每个线程一个上下文，尽管在这个简单的演示中没有体现。
    ctx: codegen::Context,

    /// 数据描述，对于数据对象的作用相当于 `ctx` 对于函数的作用。
    data_description: DataDescription,

    /// 包含 JIT 后端的模块，用于管理 JIT 编译后的
    /// 函数。
    module: JITModule,

    /// 类型检查器和函数签名注册表
    type_checker: TypeChecker,
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
        let mut builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());
        
        // 通过模块化注册表注册内置函数
        runtime::register_builtins(&mut builder);

        let module = JITModule::new(builder);
        Self {
            builder_context: FunctionBuilderContext::new(),
            ctx: module.make_context(),
            data_description: DataDescription::new(),
            module,
            type_checker: TypeChecker::new(),
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
            self.ctx.func.signature.params.push(AbiParam::new(to_cranelift_type(ty)));
        }

        // 将返回类型添加到函数签名中
        self.ctx.func.signature.returns.push(AbiParam::new(to_cranelift_type(&the_return.1)));

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
            current_func_ret_type: to_cranelift_type(&the_return.1),
            string_counter: 0,
            type_checker: &self.type_checker,
            dynamic_arrays: Vec::new(),
        };

        // 逐条翻译函数体语句
        for expr in stmts {
            trans.translate_expr(expr);
        }
        
        // 读取返回变量的值并生成 return 指令
        let (return_variable, _return_ty) = trans.variables.get(&the_return.0).expect("return variable not defined");
        let return_value = trans.builder.use_var(*return_variable);

        // 释放未返回的动态数组
        let mut drop_sig = trans.module.make_signature();
        drop_sig.params.push(AbiParam::new(types::I64));
        drop_sig.returns.push(AbiParam::new(types::I64)); // return 0
        let drop_callee = trans.module.declare_function("array_drop", Linkage::Import, &drop_sig).unwrap();
        let drop_local_callee = trans.module.declare_func_in_func(drop_callee, trans.builder.func);

        let dynamic_arrays = trans.dynamic_arrays.clone();
        for var in dynamic_arrays {
            if var != *return_variable {
                let val = trans.builder.use_var(var);
                trans.builder.ins().call(drop_local_callee, &[val]);
            }
        }

        trans.builder.ins().return_(&[return_value]);
        
        // 完成函数构建
        trans.builder.finalize();
        Ok(())
    }
}

fn to_cranelift_type(t: &FrontendType) -> types::Type {
    match t {
        FrontendType::I8 => types::I8,
        FrontendType::I16 => types::I16,
        FrontendType::I32 => types::I32,
        FrontendType::I64 => types::I64,
        FrontendType::I128 => types::I128,
        FrontendType::F32 => types::F32,
        FrontendType::F64 => types::F64,
        FrontendType::String => types::I64, // 指针
        FrontendType::Complex64 => types::I64, // 打包的 2xf32
        FrontendType::Complex128 => types::I128, // 打包的 2xf64
        FrontendType::Array(_, _) => types::I64, // 指针
        FrontendType::DynamicArray(_) => types::I64, // 指向 DynamicArray 结构体的指针
    }
}

fn is_complex(t: &FrontendType) -> bool {
    matches!(t, FrontendType::Complex64 | FrontendType::Complex128)
}

enum BinOp {
    Add, Sub, Mul, Div
}

struct FunctionTranslator<'a> {
    builder: FunctionBuilder<'a>,    // 函数构建器
    variables: HashMap<String, (Variable, FrontendType)>,    // 变量映射表
    module: &'a mut JITModule,              // JIT模块引用
    current_func_name: String,              // 当前函数名
    current_func_ret_type: types::Type,    // 当前函数返回类型
    string_counter: usize,
    type_checker: &'a TypeChecker,
    dynamic_arrays: Vec<Variable>,
}

impl<'a> FunctionTranslator<'a> {
    fn translate_expr(&mut self, expr: Expr) -> Value {         //梯度下降翻译
        match expr {
            Expr::Literal(val, ty) => {      //// 翻译字面量
                let cl_ty: types::Type = to_cranelift_type(&ty);
                match ty {
                    FrontendType::F32 => self.builder.ins().f32const(val.parse::<f32>().unwrap()),
                    FrontendType::F64 => self.builder.ins().f64const(val.parse::<f64>().unwrap()),
                    _ => {
                         let int_val = val.parse::<i128>().unwrap();
                         if cl_ty == types::I128 {
                             // 暂时将 i128 字面量视为 i64，因为 iconst 支持 Imm64。
                             // 真正的 i128 支持需要从两个 i64 构建值。
                             InstBuilder::iconst(self.builder.ins(), cl_ty, int_val as i64) 
                         } else {
                             InstBuilder::iconst(self.builder.ins(), cl_ty, int_val as i64)
                         }
                    }
                }
            }

            Expr::Add(lhs, rhs) => {
                let ty = type_checker::infer_type(&lhs, &|n| self.variables.get(n).map(|(_, t)| t.clone()));
                if is_complex(&ty) {
                    self.translate_complex_binop(*lhs, *rhs, BinOp::Add)
                } else {
                    self.translate_binary_op(*lhs, *rhs, |b, l, r| {
                        let ty = b.func.dfg.value_type(l);
                        if ty.is_float() { b.ins().fadd(l, r) } else { b.ins().iadd(l, r) }
                    })
                }
            },
            Expr::Sub(lhs, rhs) => {
                let ty = type_checker::infer_type(&lhs, &|n| self.variables.get(n).map(|(_, t)| t.clone()));
                if is_complex(&ty) {
                    self.translate_complex_binop(*lhs, *rhs, BinOp::Sub)
                } else {
                    self.translate_binary_op(*lhs, *rhs, |b, l, r| {
                        let ty = b.func.dfg.value_type(l);
                        if ty.is_float() { b.ins().fsub(l, r) } else { b.ins().isub(l, r) }
                    })
                }
            },
            Expr::Mul(lhs, rhs) => {
                let ty = type_checker::infer_type(&lhs, &|n| self.variables.get(n).map(|(_, t)| t.clone()));
                if is_complex(&ty) {
                    self.translate_complex_binop(*lhs, *rhs, BinOp::Mul)
                } else {
                    self.translate_binary_op(*lhs, *rhs, |b, l, r| {
                        let ty = b.func.dfg.value_type(l);
                        if ty.is_float() { b.ins().fmul(l, r) } else { b.ins().imul(l, r) }
                    })
                }
            },
            Expr::Div(lhs, rhs) => {
                let ty = type_checker::infer_type(&lhs, &|n| self.variables.get(n).map(|(_, t)| t.clone()));
                if is_complex(&ty) {
                    self.translate_complex_binop(*lhs, *rhs, BinOp::Div)
                } else {
                    self.translate_binary_op(*lhs, *rhs, |b, l, r| {
                        let ty = b.func.dfg.value_type(l);
                        if ty.is_float() { b.ins().fdiv(l, r) } else { b.ins().udiv(l, r) }
                    })
                }
            },

            Expr::Eq(lhs, rhs) => self.translate_cmp(*lhs, *rhs, IntCC::Equal, FloatCC::Equal),
            Expr::Ne(lhs, rhs) => self.translate_cmp(*lhs, *rhs, IntCC::NotEqual, FloatCC::NotEqual),
            Expr::Lt(lhs, rhs) => self.translate_cmp(*lhs, *rhs, IntCC::SignedLessThan, FloatCC::LessThan),
            Expr::Le(lhs, rhs) => self.translate_cmp(*lhs, *rhs, IntCC::SignedLessThanOrEqual, FloatCC::LessThanOrEqual),
            Expr::Gt(lhs, rhs) => self.translate_cmp(*lhs, *rhs, IntCC::SignedGreaterThan, FloatCC::GreaterThan),
            Expr::Ge(lhs, rhs) => self.translate_cmp(*lhs, *rhs, IntCC::SignedGreaterThanOrEqual, FloatCC::GreaterThanOrEqual),

            Expr::Call(name, args) => self.translate_call(name, args),
            Expr::GlobalDataAddr(name) => self.translate_global_data_addr(name),
            Expr::StringLiteral(s) => self.translate_string_literal(s),
            Expr::ComplexLiteral(re, im, ty) => self.translate_complex_literal(re, im, ty),
            Expr::ArrayLiteral(elems, ty) => self.translate_array_literal(elems, ty),
            Expr::DynamicArrayLiteral(elems, ty) => self.translate_dynamic_array_literal(elems, ty),
            Expr::Index(base, idx) => self.translate_index(*base, *idx),
            Expr::Identifier(name) => {
                let (variable, _) = self.variables.get(&name).expect("variable not defined");
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
                self.translate_cast(val, to_cranelift_type(&target_ty))
            }
        }
    }
    
    fn translate_binary_op<F>(&mut self, lhs: Expr, rhs: Expr, op: F) -> Value 
    where F: Fn(&mut FunctionBuilder, Value, Value) -> Value {
        let l_val = self.translate_expr(lhs);    // 先把左边的表达式翻译完，拿到结果线头
        let r_val = self.translate_expr(rhs);    // 再把右边的表达式翻译完，拿到结果线头
        let (l_promoted, r_promoted) = self.promote_operands(l_val, r_val);
        //如果左边是 i32，右边是 i64，要把左边“拉长”成 i64
        op(&mut self.builder, l_promoted, r_promoted)  // 生成真正的加法指令
    }   
    
    fn promote_operands(&mut self, lhs: Value, rhs: Value) -> (Value, Value) {
        let l_ty = self.builder.func.dfg.value_type(lhs);
        let r_ty = self.builder.func.dfg.value_type(rhs);
        
        if l_ty == r_ty {
            return (lhs, rhs);
        }
        
        // 隐式提升：int -> 更宽的 int，float -> 更宽的 float。
        // 没有隐式的 int <-> float 转换。
        
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
        // 如果缺少 bint，则使用 select 代替
        let one = InstBuilder::iconst(self.builder.ins(), types::I64, 1);
        let zero = InstBuilder::iconst(self.builder.ins(), types::I64, 0);
        self.builder.ins().select(bool_res, one, zero)
    }
    ///变量赋值
    fn translate_assign(&mut self, name: String, expr: Expr) -> Value {
        let new_value = self.translate_expr(expr);
        let (variable, ty) = {
            let (v, t) = self.variables.get(&name).unwrap();
            (*v, t.clone())
        };
        
        let target_ty = to_cranelift_type(&ty);
        let val_ty = self.builder.func.dfg.value_type(new_value);
        
        let final_value = if val_ty != target_ty {
            self.translate_cast(new_value, target_ty)
        } else {
            new_value
        };
        
        self.builder.def_var(variable, final_value);

        if matches!(ty, FrontendType::DynamicArray(_)) {
            if !self.dynamic_arrays.contains(&variable) {
                self.dynamic_arrays.push(variable);
            }
        }

        final_value
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
        let mut then_return = InstBuilder::iconst(self.builder.ins(), types::I64, 0); // 默认值
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
        
        // 显式转换 else 结果以匹配 then 结果类型（简单统一）
        let else_return_cast = if then_ty != self.builder.func.dfg.value_type(else_return) {
             // 为简单起见，我们直接使用 else_return，希望一切顺利或依赖验证错误。
             // 在此处实现正确的转换需要访问 self.translate_cast，这需要 &mut self。
             // 我们可以调用它！
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
        
        let signature = self.type_checker.resolve_func(&name);

        let mut arg_values = Vec::new();
        for arg in args {
            // 推断类型以检查它是否为数组
            let arg_ty = type_checker::infer_type(&arg, &|n| self.variables.get(n).map(|(_, t)| t.clone()));
            
            let val = self.translate_expr(arg);
            
            let should_expand = if let FrontendType::Array(_, _) = arg_ty {
                 // 仅对外部函数将数组展开为 (ptr, len)
                 signature.map(|s| s.is_external).unwrap_or(false)
            } else {
                 false
            };

            // DynamicArray 作为单个指针传递，不需要展开。
            
            if should_expand {
                 arg_values.push(val);
                 sig.params.push(AbiParam::new(self.builder.func.dfg.value_type(val)));
                 
                 let len = if let FrontendType::Array(_, l) = arg_ty { l } else { 0 };
                 let len_val = self.builder.ins().iconst(types::I64, len as i64);
                 arg_values.push(len_val);
                 sig.params.push(AbiParam::new(types::I64));
            } else {
                arg_values.push(val);
                sig.params.push(AbiParam::new(self.builder.func.dfg.value_type(val)));
            }
        }

        // 返回类型？
        let ret_ty = if let Some(s) = signature {
            to_cranelift_type(&s.ret)
        } else if name == self.current_func_name {
             self.current_func_ret_type
        } else if name == "printf" || name == "puts" {
             types::I32
        } else {
             // 对于未知函数，假设为 I64
             types::I64
        };
        sig.returns.push(AbiParam::new(ret_ty));

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

    fn translate_string_literal(&mut self, s: String) -> Value {
        self.string_counter += 1;
        let name = format!("str_{}_{}", self.current_func_name, self.string_counter);
        
        let data_id = self.module.declare_data(
            &name,
            Linkage::Local,
            false,
            false
        ).unwrap();
        
        let mut data_ctx = DataDescription::new();
        // 以 Null 结尾的字符串，以兼容 printf
        let mut bytes = s.as_bytes().to_vec();
        bytes.push(0);
        data_ctx.define(bytes.into_boxed_slice());
        
        self.module.define_data(data_id, &data_ctx).unwrap();
        
        let local_id = self.module.declare_data_in_func(data_id, self.builder.func);
        let pointer = self.module.target_config().pointer_type();
        self.builder.ins().symbol_value(pointer, local_id)
    }

    fn translate_complex_literal(&mut self, re: f64, im: f64, ty: FrontendType) -> Value {
         match ty {
             FrontendType::Complex64 => {
                 let re_bits = (re as f32).to_bits() as u64;
                 let im_bits = (im as f32).to_bits() as u64;
                 let val = re_bits | (im_bits << 32);
                 self.builder.ins().iconst(types::I64, val as i64)
             },
             FrontendType::Complex128 => {
                 let re_bits = re.to_bits();
                 let im_bits = im.to_bits();
                 
                 let ss = self.builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 16, 4));
                  let low = self.builder.ins().iconst(types::I64, re_bits as i64);
                 let high = self.builder.ins().iconst(types::I64, im_bits as i64);
                 
                 self.builder.ins().stack_store(low, ss, 0);
                 self.builder.ins().stack_store(high, ss, 8);
                 self.builder.ins().stack_load(types::I128, ss, 0)
             },
             _ => panic!("Invalid complex type"),
         }
    }

    fn translate_array_literal(&mut self, elems: Vec<Expr>, _ty: FrontendType) -> Value {
        // 重新推断类型，因为解析器使用占位符
        let actual_ty = if elems.is_empty() {
             FrontendType::Array(Box::new(FrontendType::I64), 0)
        } else {
             let elem_ty = type_checker::infer_type(&elems[0], &|n| self.variables.get(n).map(|(_, t)| t.clone()));
             FrontendType::Array(Box::new(elem_ty), elems.len())
        };

        let (elem_ty, len) = match actual_ty {
            FrontendType::Array(t, l) => (*t, l),
            _ => panic!("Expected array type"),
        };
        
        let cl_elem_ty = to_cranelift_type(&elem_ty);
        let elem_size = cl_elem_ty.bytes();
        let total_size = elem_size * (len as u32);
        
        // 对元素使用自然对齐
        let align_shift = (elem_size as f64).log2().ceil() as u8;
        
        let slot = self.builder.create_sized_stack_slot(StackSlotData {
            kind: StackSlotKind::ExplicitSlot,
            size: total_size,
            align_shift,
        });
        
        for (i, elem) in elems.into_iter().enumerate() {
            let val = self.translate_expr(elem);
            let offset = (i as i32) * (elem_size as i32);
            self.builder.ins().stack_store(val, slot, offset);
        }
        
        self.builder.ins().stack_addr(types::I64, slot, 0)
    }

    fn translate_dynamic_array_literal(&mut self, elems: Vec<Expr>, _ty: FrontendType) -> Value {
        // 目前我们只支持 i64 动态数组
        let mut sig = self.module.make_signature();
        sig.returns.push(AbiParam::new(types::I64));
        
        let callee = self.module.declare_function("array_new_i64", Linkage::Import, &sig).unwrap();
        let local_callee = self.module.declare_func_in_func(callee, self.builder.func);
        let call = self.builder.ins().call(local_callee, &[]);
        let arr_ptr = self.builder.inst_results(call)[0];
        
        // 推送元素
        if !elems.is_empty() {
            let mut push_sig = self.module.make_signature();
            push_sig.params.push(AbiParam::new(types::I64)); // arr_ptr
            push_sig.params.push(AbiParam::new(types::I64)); // elem
            push_sig.returns.push(AbiParam::new(types::I64)); // return 0
            
            let push_callee = self.module.declare_function("array_push", Linkage::Import, &push_sig).unwrap();
            let push_local_callee = self.module.declare_func_in_func(push_callee, self.builder.func);
            
            for elem in elems {
                let val = self.translate_expr(elem);
                // 如果需要，提升/转换为 I64
                let val_i64 = self.translate_cast(val, types::I64);
                self.builder.ins().call(push_local_callee, &[arr_ptr, val_i64]);
            }
        }
        
        arr_ptr
    }

    fn translate_index(&mut self, base: Expr, idx: Expr) -> Value {
        let base_ty = type_checker::infer_type(&base, &|n| self.variables.get(n).map(|(_, t)| t.clone()));
        let (elem_ty, len, is_dynamic) = match base_ty {
            FrontendType::Array(t, l) => (*t, l, false),
            FrontendType::DynamicArray(t) => (*t, 0, true),
            FrontendType::String => (FrontendType::I8, 0, false), // 字符串暂无边界检查
            _ => panic!("Cannot index non-array type: {:?}", base_ty),
        };
        
        let base_val = self.translate_expr(base);
        let idx_val = self.translate_expr(idx);
        
        let cl_elem_ty = to_cranelift_type(&elem_ty);
        
        let idx_val_i64 = if self.builder.func.dfg.value_type(idx_val) != types::I64 {
             self.builder.ins().uextend(types::I64, idx_val)
        } else {
             idx_val
        };
        
        if is_dynamic {
             // 对于动态数组，我们需要调用 array_get_ptr
             let mut sig = self.module.make_signature();
             sig.params.push(AbiParam::new(types::I64)); // arr_ptr
             sig.params.push(AbiParam::new(types::I64)); // index
             sig.returns.push(AbiParam::new(types::I64)); // 元素指针
             
             let callee = self.module.declare_function("array_get_ptr", Linkage::Import, &sig).unwrap();
             let local_callee = self.module.declare_func_in_func(callee, self.builder.func);
             let call = self.builder.ins().call(local_callee, &[base_val, idx_val_i64]);
             let addr = self.builder.inst_results(call)[0];
             
             // 如果 addr 为空（索引越界），则触发陷阱
             self.builder.ins().trapz(addr, TrapCode::unwrap_user(1));
             
             self.builder.ins().load(cl_elem_ty, MemFlags::new(), addr, 0)
        } else {
            let elem_size = cl_elem_ty.bytes() as i64;
            // 边界检查
            if len > 0 {
                 let len_val = self.builder.ins().iconst(types::I64, len as i64);
                 // idx < 0 || idx >= len (无符号检查覆盖两者)
                 // 如果 idx >= len，触发陷阱。
                 let out_of_bounds = self.builder.ins().icmp(IntCC::UnsignedGreaterThanOrEqual, idx_val_i64, len_val);
                 self.builder.ins().trapnz(out_of_bounds, TrapCode::unwrap_user(1));
            }
            
            let offset = self.builder.ins().imul_imm(idx_val_i64, elem_size);
            let addr = self.builder.ins().iadd(base_val, offset);
            
            self.builder.ins().load(cl_elem_ty, MemFlags::new(), addr, 0)
        }
    }

    fn translate_complex_binop(&mut self, lhs: Expr, rhs: Expr, op: BinOp) -> Value {
        let l_val = self.translate_expr(lhs);
        let r_val = self.translate_expr(rhs);
        // 假设类型匹配（infer_type 确保这一点，否则我们会 panic 或提升）。
        let ty = self.builder.func.dfg.value_type(l_val);
        
        if ty == types::I64 { // Complex64
            // 解包 l
            let l_re_bits = self.builder.ins().ireduce(types::I32, l_val); // 低 32 位
            let l_val_shifted = self.builder.ins().ushr_imm(l_val, 32);
            let l_im_bits = self.builder.ins().ireduce(types::I32, l_val_shifted);
            let l_re = self.builder.ins().bitcast(types::F32, MemFlags::new(), l_re_bits);
            let l_im = self.builder.ins().bitcast(types::F32, MemFlags::new(), l_im_bits);
            
            // 解包 r
            let r_re_bits = self.builder.ins().ireduce(types::I32, r_val);
            let r_val_shifted = self.builder.ins().ushr_imm(r_val, 32);
            let r_im_bits = self.builder.ins().ireduce(types::I32, r_val_shifted);
            let r_re = self.builder.ins().bitcast(types::F32, MemFlags::new(), r_re_bits);
            let r_im = self.builder.ins().bitcast(types::F32, MemFlags::new(), r_im_bits);
            
            let (res_re, res_im) = match op {
                BinOp::Add => (self.builder.ins().fadd(l_re, r_re), self.builder.ins().fadd(l_im, r_im)),
                BinOp::Sub => (self.builder.ins().fsub(l_re, r_re), self.builder.ins().fsub(l_im, r_im)),
                BinOp::Mul => {
                    let ac = self.builder.ins().fmul(l_re, r_re);
                    let bd = self.builder.ins().fmul(l_im, r_im);
                    let ad = self.builder.ins().fmul(l_re, r_im);
                    let bc = self.builder.ins().fmul(l_im, r_re);
                    (self.builder.ins().fsub(ac, bd), self.builder.ins().fadd(ad, bc))
                },
                BinOp::Div => {
                    let c2 = self.builder.ins().fmul(r_re, r_re);
                    let d2 = self.builder.ins().fmul(r_im, r_im);
                    let denom = self.builder.ins().fadd(c2, d2);
                    let ac = self.builder.ins().fmul(l_re, r_re);
                    let bd = self.builder.ins().fmul(l_im, r_im);
                    let num_re = self.builder.ins().fadd(ac, bd);
                    let bc = self.builder.ins().fmul(l_im, r_re);
                    let ad = self.builder.ins().fmul(l_re, r_im);
                    let num_im = self.builder.ins().fsub(bc, ad);
                    (self.builder.ins().fdiv(num_re, denom), self.builder.ins().fdiv(num_im, denom))
                }
            };
            
            // 重新打包
            let res_re_bits = self.builder.ins().bitcast(types::I32, MemFlags::new(), res_re);
            let res_im_bits = self.builder.ins().bitcast(types::I32, MemFlags::new(), res_im);
            
            let res_re_i64 = self.builder.ins().uextend(types::I64, res_re_bits);
            let res_im_i64 = self.builder.ins().uextend(types::I64, res_im_bits);
            let res_im_shifted = self.builder.ins().ishl_imm(res_im_i64, 32);
            self.builder.ins().bor(res_re_i64, res_im_shifted)
            
        } else if ty == types::I128 { // Complex128
            let ss = self.builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 16, 4));

            // 解包 l_val
            self.builder.ins().stack_store(l_val, ss, 0);
            let l_re = self.builder.ins().stack_load(types::F64, ss, 0);
            let l_im = self.builder.ins().stack_load(types::F64, ss, 8);
            
            // 解包 r_val
            self.builder.ins().stack_store(r_val, ss, 0);
            let r_re = self.builder.ins().stack_load(types::F64, ss, 0);
            let r_im = self.builder.ins().stack_load(types::F64, ss, 8);
            
            let (res_re, res_im) = match op {
                BinOp::Add => (self.builder.ins().fadd(l_re, r_re), self.builder.ins().fadd(l_im, r_im)),
                BinOp::Sub => (self.builder.ins().fsub(l_re, r_re), self.builder.ins().fsub(l_im, r_im)),
                BinOp::Mul => {
                    let ac = self.builder.ins().fmul(l_re, r_re);
                    let bd = self.builder.ins().fmul(l_im, r_im);
                    let ad = self.builder.ins().fmul(l_re, r_im);
                    let bc = self.builder.ins().fmul(l_im, r_re);
                    (self.builder.ins().fsub(ac, bd), self.builder.ins().fadd(ad, bc))
                },
                BinOp::Div => {
                    let c2 = self.builder.ins().fmul(r_re, r_re);
                    let d2 = self.builder.ins().fmul(r_im, r_im);
                    let denom = self.builder.ins().fadd(c2, d2);
                    let ac = self.builder.ins().fmul(l_re, r_re);
                    let bd = self.builder.ins().fmul(l_im, r_im);
                    let num_re = self.builder.ins().fadd(ac, bd);
                    let bc = self.builder.ins().fmul(l_im, r_re);
                    let ad = self.builder.ins().fmul(l_re, r_im);
                    let num_im = self.builder.ins().fsub(bc, ad);
                    (self.builder.ins().fdiv(num_re, denom), self.builder.ins().fdiv(num_im, denom))
                }
            };
            
            // 重新打包
            self.builder.ins().stack_store(res_re, ss, 0);
            self.builder.ins().stack_store(res_im, ss, 8);
            self.builder.ins().stack_load(types::I128, ss, 0)
        } else {
            panic!("不支持的复数类型 IR: {:?}", ty);
        }
    }
}

/// 在 JIT 编译开始前 扫描并声明所有变量
fn declare_variables(
    builder: &mut FunctionBuilder,
    params: &[(String, FrontendType)],
    stmts: &[Expr],
    entry_block: Block,
    return_info: &(String, FrontendType),
) -> HashMap<String, (Variable, FrontendType)> {
    let mut variables = HashMap::new();
    
    // - 注册 ：为每个函数参数创建一个 Cranelift 变量（ declare_var ）。
    // - 绑定 ：把函数的 入口参数值 （ block_params ）赋给这个变量（ def_var ）。
    for (i, (name, ty)) in params.iter().enumerate() {
        let val = builder.block_params(entry_block)[i];
        let var = builder.declare_var(to_cranelift_type(ty));
        variables.insert(name.clone(), (var, ty.clone()));
        builder.def_var(var, val);
        
        // 跟踪作为参数传递的动态数组（调用者拥有它们？通常调用者拥有，但在 Toy 中，如果是最后一个拥有者，我们可能希望被调用者丢弃。
        // 为简单起见，我们假设被调用者不丢弃参数，只丢弃局部变量。
        // 实际上，如果我们想要 RAII，我们应该决定所有权。
    }
    
    // 声明返回值变量
    let (ret_name, ret_ty) = return_info;
    if !variables.contains_key(ret_name) {
        let cl_ty = to_cranelift_type(ret_ty);
        let var = builder.declare_var(cl_ty);
        variables.insert(ret_name.clone(), (var, ret_ty.clone()));
        
        // 将返回变量初始化为 0 或等效值
        let zero = match ret_ty {
            FrontendType::F32 => builder.ins().f32const(0.0),
            FrontendType::F64 => builder.ins().f64const(0.0),
            _ => InstBuilder::iconst(builder.ins(), cl_ty, 0),
        };
        builder.def_var(var, zero);
    }
    
    // 扫描语句中的隐式变量
    for expr in stmts {
        declare_variables_in_stmt(builder, &mut variables, expr);
    }

    variables
}

/// 递归扫描表达式中的变量声明
fn declare_variables_in_stmt(
    builder: &mut FunctionBuilder,
    variables: &mut HashMap<String, (Variable, FrontendType)>,
    expr: &Expr,
) {
    match *expr {
        Expr::Assign(ref name, ref val_expr) => {
            if !variables.contains_key(name) {
                // 推断类型
                let ty = type_checker::infer_type(val_expr, &|n| variables.get(n).map(|(_, t)| t.clone()));
                let var = builder.declare_var(to_cranelift_type(&ty));
                variables.insert(name.clone(), (var, ty));
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
