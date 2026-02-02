# 移除 nalgebra 并集成 Intel MKL 的完整方案

本计划旨在从现有项目中彻底移除 `nalgebra` 依赖，并集成高性能的 Intel MKL 库，将其 `cblas_dgemm` 等函数动态注册到 Cranelift JIT 环境中。

## 1. 彻底移除 nalgebra
- **Cargo.toml**: 移除 `nalgebra` 依赖项。
- **extern_functions.rs**: 
    - 删除 `use nalgebra::{SMatrix};` 导入。
    - 删除依赖 `nalgebra` 的函数：`toy_print_matrix_2x2` 和 `toy_sum_array`。
- **jit.rs**: 移除 `JITBuilder` 中对 `sum_array` 和 `print_matrix_2x2` 的符号注册。
- **type_checker.rs**: 从 `register_builtins` 中移除相关函数的签名定义。
- **测试用例**: 删除 `integration_test.rs` 中涉及 `nalgebra` 的旧测试。

## 2. 集成 Intel MKL
- **Cargo.toml**: 添加 `intel-mkl-src` 作为依赖（用于多平台链接支持）。
- **build.rs**: 编写构建脚本，使用 `intel-mkl-src` 配置 MKL 库的链接路径（支持 Linux, Windows, macOS）。
- **extern_functions.rs**:
    - 引入 `intel-mkl-src` 提供的 FFI 绑定或手动定义 `cblas_dgemm` 的 `extern "C"` 签名。
    - 实现一个 Rust 包装函数 `toy_mkl_dgemm`，以便更安全地暴露给 JIT。

## 3. JIT 环境动态注册
- **jit.rs**:
    - 在 `JIT::default()` 中，将 `cblas_dgemm`（或包装后的函数）的地址注册到 `JITBuilder` 的符号表中。
- **type_checker.rs**:
    - 在 `register_builtins` 中添加 `cblas_dgemm` 的参数和返回类型签名，确保 JIT 编译时的类型检查。

## 4. 端到端验证与测试
- **integration_test.rs**: 新增 MKL 矩阵乘法测试，验证 JIT 代码调用 `cblas_dgemm` 的计算正确性。
- **toy.rs**: 在主程序中添加 MKL 示例代码演示。

## 5. 性能基准测试
- **jit_bench.rs**: 更新基准测试，对比 MKL DGEMM 与原实现的性能（包括二进制大小和运行时延迟的定性分析）。

## 6. 交付文档
- 提供完整的构建指南、MKL 环境变量配置说明以及跨平台兼容性说明。

是否开始执行该方案？