# Cranelift JIT Demo

Toy 语言即时编译（JIT）演示项目，基于 Cranelift 编译器后端。

在原 [bytecodealliance/cranelift-jit-demo](https://github.com/bytecodealliance/cranelift-jit-demo) 基础上进行了大幅扩展。

## 特性

- **PEG 解析器**: 完整自定义语法，支持函数定义、控制流、算术/比较运算
- **类型系统**: I8/I16/I32/I64/I128、F32/F64、String、Complex64/128、固定数组、动态数组
- **常量折叠**: AST 级编译时优化
- **所有权检查**: DynamicArray 的编译时内存安全分析（泄漏检测、use-after-drop、double-drop）
- **Cranelift JIT**: 生成原生机器码，支持递归函数
- **运行时库**: 数学函数、I/O、动态数组（基于 RAII）、Intel MKL GEMM

## 构建

```bash
# 默认构建
cargo build --release

# 启用 MKL 支持
cargo build --release --features mkl
```

## 测试

```bash
# 运行所有测试
cargo test

# 运行内置冒烟测试
cargo run -- --test

# 运行 .toy 脚本
cargo run -- examples/sin.toy
```

## 性能基准

```bash
cargo bench --bench jit_bench
cd raii_demo && cargo bench
```

## 项目结构

```
src/
  frontend.rs      PEG 解析器与 AST 定义
  jit.rs           Cranelift JIT 编译器主体
  optimizer.rs     常量折叠优化
  ownership.rs     DynamicArray 所有权检查
  type_checker.rs  类型推断与函数签名注册
  runtime/         运行时函数 (array, math, io, mkl, string)
  cli/             CLI 参数解析
  bin/toy.rs       main 入口

raii_demo/   RAII 风格 DynamicArray 实现
benches/                  Criterion 基准测试
examples/                 .toy 示例脚本
```

## 完整文档

详见 [PROJECT_GUIDE.md](PROJECT_GUIDE.md)
