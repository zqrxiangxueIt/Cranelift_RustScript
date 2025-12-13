// src/jit.rs
use cranelift::prelude::*;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Linkage, Module};

/// JIT 编译器封装结构体
pub struct JIT {
    /// 函数构建上下文，可复用以减少内存分配
    builder_context: FunctionBuilderContext,
    /// 代码生成上下文
    ctx: codegen::Context,
    /// JIT 模块
    module: JITModule,
}

impl Default for JIT {
    fn default() -> Self {
        // [修改] 适配 Cranelift 0.125+ API
        // JITBuilder::new 现在返回 Result，必须使用 unwrap() 处理
        let builder = JITBuilder::new(cranelift_module::default_libcall_names())
            .unwrap();

        // 初始化 JITModule
        let module = JITModule::new(builder);

        Self {
            builder_context: FunctionBuilderContext::new(),
            ctx: module.make_context(),
            module,
        }
    }
}

impl JIT {
    /// 编译并执行 AST
    pub fn compile_and_run(&mut self, stmts: Vec<crate::ast::Stmt>) -> Result<i64, String> {
        // 1. 设置函数签名：无参数，返回 I64
        let mut sig = self.module.make_signature();
        sig.returns.push(AbiParam::new(types::I64));

        // 2. 在模块中声明函数 "main"
        let func_id = self.module.declare_function("main", Linkage::Export, &sig)
            .map_err(|e| e.to_string())?;

        self.ctx.func.signature = sig;

        // 3. 构建 IR
        {
            let mut builder = FunctionBuilder::new(&mut self.ctx.func, &mut self.builder_context);
            // 创建入口基本块 (Entry Block)
            let entry_block = builder.create_block();
            // 为入口块添加参数（如果函数有参数的话，这里是空的）
            builder.append_block_params_for_function_params(entry_block);
            // 切换插入点到入口块
            builder.switch_to_block(entry_block);
            // 封印入口块（表示不会再有前驱跳转到此块，允许 SSA 优化）
            builder.seal_block(entry_block);

            // 启动翻译器
            let mut translator = crate::frontend::FunctionTranslator::new(builder, &mut self.module);
            let mut return_value = translator.translate_stmts(stmts);

            // 默认返回 0
            if return_value.is_none() {
                return_value = Some(translator.builder.ins().iconst(types::I64, 0));
            }

            // 发射返回指令
            translator.builder.ins().return_(&[return_value.unwrap()]);
            // 完成函数构建
            translator.builder.finalize();
        }

        // 4. 定义函数（触发代码生成）
        self.module.define_function(func_id, &mut self.ctx)
            .map_err(|e| e.to_string())?;

        // 5. 清理上下文以便下次使用
        self.module.clear_context(&mut self.ctx);

        // 6. 终结定义并获取机器码
        self.module.finalize_definitions()
            .map_err(|e| e.to_string())?;

        // 7. 获取生成的机器码指针
        let code_ptr = self.module.get_finalized_function(func_id);

        // 8. 执行机器码
        // SAFETY: 我们确定生成的代码签名是 () -> i64，且内存是可执行的
        let code_fn = unsafe { std::mem::transmute::<_, extern "C" fn() -> i64>(code_ptr) };
        let result = code_fn();

        Ok(result)
    }
}