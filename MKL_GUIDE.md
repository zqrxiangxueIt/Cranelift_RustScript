# Intel MKL 集成与 JIT 注册指南

本指南详细介绍了如何在该 Cranelift JIT 演示项目中集成 Intel MKL 并移除 nalgebra 依赖。

## 1. 环境要求
- **Intel MKL**: 推荐安装 Intel OneAPI Base Toolkit。
- **平台支持**: Windows, Linux, macOS。
- **Rust 版本**: 1.80+ (支持 Rust 2024 edition)。

## 2. 构建与运行
项目使用 `intel-mkl-src` 自动处理链接。如果系统中未检测到 MKL，它将尝试从 GitHub 下载二进制文件。

```bash
# 运行集成测试
cargo test --test integration_test

# 运行主程序示例
cargo run --bin toy

# 运行基准测试
cargo bench
```

## 3. JIT 注册 API 使用示例
在 `toy` 语言中，可以通过 `toy_mkl_dgemm` 函数直接调用 MKL 的 DGEMM。

### 函数签名
```rust
fn toy_mkl_dgemm(
    m: i64, n: i64, k: i64, 
    alpha: f64, a: [f64; SIZE_A], 
    beta: f64, b: [f64; SIZE_B], 
    c: [f64; SIZE_C]
)
```

### 示例代码
```rust
fn matrix_multiply(c: [f64; 4]) -> (r: i64) {
    a = [1.0, 2.0, 3.0, 4.0]
    b = [5.0, 6.0, 7.0, 8.0]
    // 执行 C = 1.0 * A * B + 0.0 * C
    toy_mkl_dgemm(2, 2, 2, 1.0, a, 0.0, b, c)
    r = 0
}
```

## 4. 平台差异处理
- **Windows**: 使用 `mkl-static-lp64-seq` 特性进行静态链接，无需手动配置 DLL 路径。
- **Linux/macOS**: 同样采用静态链接以确保生成的二进制文件可移植。
- **数据模型**: 默认使用 `lp64` (32-bit int)，与大多数 64 位系统的 BLAS 习惯一致。

## 5. 故障排查
- **链接失败 (LNK1181)**: 确保 `Cargo.toml` 中 `intel-mkl-src` 的特性配置正确。如果下载失败，请手动设置 `MKLROOT` 环境变量。
- **JIT 运行时崩溃**: 检查传递给 `toy_mkl_dgemm` 的数组大小是否与 `m, n, k` 匹配。BLAS 不进行边界检查。

## 6. CI 脚本建议
在 CI 中集成时，建议缓存 `target` 目录以避免重复下载 MKL 库。
```yaml
- name: Cache MKL
  uses: actions/cache@v3
  with:
    path: target
    key: ${{ runner.os }}-mkl
```
