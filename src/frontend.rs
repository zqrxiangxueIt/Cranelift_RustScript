// src/frontend.rs
use cranelift::prelude::*;
use cranelift_module::Module;
use std::collections::HashMap;
use crate::ast::{Expr, Op, Stmt};

/// 函数翻译器上下文
pub struct FunctionTranslator<'a, M: Module> {
    /// Cranelift 的 IR 构建器
    pub builder: FunctionBuilder<'a>,
    /// 模块引用，用于声明数据或函数调用
    module: &'a mut M,
    /// 变量名到 Cranelift Variable 句柄的映射表
    variables: HashMap<String, Variable>,
    // [修改] 移除了 var_index，因为新版 Cranelift 自动管理变量索引
}

impl<'a, M: Module> FunctionTranslator<'a, M> {
    pub fn new(builder: FunctionBuilder<'a>, module: &'a mut M) -> Self {
        Self {
            builder,
            module,
            variables: HashMap::new(),
        }
    }

    /// 翻译语句序列，返回最后一条语句的值作为函数返回值
    pub fn translate_stmts(&mut self, stmts: Vec<Stmt>) -> Option<Value> {
        let mut last_val = None;
        for stmt in stmts {
            last_val = Some(self.translate_stmt(stmt));
        }
        last_val
    }

    fn translate_stmt(&mut self, stmt: Stmt) -> Value {
        match stmt {
            Stmt::Expr(expr) => self.translate_expr(expr),
        }
    }

    fn translate_expr(&mut self, expr: Expr) -> Value {
        match expr {
            Expr::Literal(n) => {
                // 生成 iconst 指令：将立即数加载到 SSA 值中
                self.builder.ins().iconst(types::I64, n)
            }
            Expr::BinaryOp { op, lhs, rhs } => {
                // 递归翻译左右子树
                let lhs_val = self.translate_expr(*lhs);
                let rhs_val = self.translate_expr(*rhs);

                // 发射对应的算术指令
                match op {
                    Op::Add => self.builder.ins().iadd(lhs_val, rhs_val),
                    Op::Sub => self.builder.ins().isub(lhs_val, rhs_val),
                    Op::Mul => self.builder.ins().imul(lhs_val, rhs_val),
                    Op::Div => self.builder.ins().sdiv(lhs_val, rhs_val), // 有符号除法
                }
            }
            Expr::Assign { name, value } => {
                let val = self.translate_expr(*value);
                let var = self.get_or_create_var(&name);
                // 定义变量的新值 (SSA Define)
                self.builder.def_var(var, val);
                val // 赋值表达式返回该值
            }
            Expr::Identifier(name) => {
                if let Some(var) = self.variables.get(&name) {
                    // 使用变量 (SSA Use)
                    self.builder.use_var(*var)
                } else {
                    // 在正式编译器中应报错 "Undefined variable"，此处为演示方便返回 0
                    eprintln!("Warning: Use of undefined variable '{}', defaulting to 0", name);
                    self.builder.ins().iconst(types::I64, 0)
                }
            }
        }
    }

    /// 辅助函数：管理变量符号表
    fn get_or_create_var(&mut self, name: &str) -> Variable {
        if let Some(var) = self.variables.get(name) {
            *var
        } else {
            // [修改] 适配 Cranelift 0.125+ API
            // 不再需要手动 new Variable，直接告诉 builder 声明一个 I64 类型的变量，它会返回句柄
            let var = self.builder.declare_var(types::I64);
            self.variables.insert(name.to_string(), var);
            var
        }
    }
}