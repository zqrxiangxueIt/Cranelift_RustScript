#!/bin/bash
# Miri 测试脚本
# 用于检测 Unsafe 代码中的 UB、对齐错误和内存泄漏

echo "Running tests with Miri..."
cargo +nightly miri test
