#!/bin/bash
# Valgrind 测试脚本
# 用于检测运行时内存泄漏与非法访问

echo "Building tests..."
cargo test --no-run

# 查找最新的测试二进制文件
TEST_BIN=$(find target/debug/deps -maxdepth 1 -executable -name "raii_demo-*" | head -n 1)

if [ -z "$TEST_BIN" ]; then
    echo "Test binary not found!"
    exit 1
fi

echo "Running Valgrind on $TEST_BIN..."
valgrind --leak-check=full --show-leak-kinds=all --error-exitcode=1 "$TEST_BIN"
