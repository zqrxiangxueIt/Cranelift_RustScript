# 基于 Rust RAII 的动态数组与内存安全技术方案

本方案旨在为高性能编译器/运行时系统提供一套内存安全的动态数组实现标准。该方案基于 Rust 的所有权模型（Ownership）与资源获取即初始化（RAII）原则，确保在实现底层手动内存管理的同时，向上传递零成本、内存安全的抽象接口。

## 1. 预期实现效果

| 关键指标 | 预期表现 | 性能目标 |
| :--- | :--- | :--- |
| **容量管理** | 支持指数级增长策略（Amortized O(1)），减少重分配次数。支持 `shrink_to_fit` 释放闲置内存。 | 重分配耗时 < 2x `memcpy` 耗时 |
| **内存分配** | 接入 `GlobalAlloc` 或自定义 `Allocator`，支持对齐（Alignment）与大页优化。 | 分配开销近似 `malloc`，无额外元数据开销 |
| **地址安全** | 严格的生命周期绑定，通过借用检查器防止悬垂指针（Dangling Pointer）与使用后释放（Use-After-Free）。 | 编译期 100% 拦截生命周期错误 |
| **溢出防护** | 物理内存耗尽（OOM）时提供 `try_reserve` 接口处理错误，而非直接 Panic；数组索引访问提供边界检查。 | 边界检查在热循环中可被自动向量化消除 |
| **清理机制** | 作用域结束自动释放内存（Drop），并在 Panic 时保证异常安全（Exception Safety），防止内存泄漏。 | 析构开销 = 元素析构之和 + 1次 `free` |

## 2. 技术路线

### 2.1 核心数据结构设计
采用“三字结构”指针管理堆内存，利用 `NonNull<T>` 实现协变与空指针优化。

```rust
pub struct DynamicArray<T, A: Allocator = Global> {
    ptr: NonNull<T>,      // 堆内存起始地址（非空、协变）
    cap: usize,           // 当前分配的容量
    len: usize,           // 当前有效元素数量
    alloc: A,             // 内存分配器实例（零尺寸优化）
    _marker: PhantomData<T>, // 标记拥有 T 的所有权
}
```

### 2.2 内存布局与分配 (Layout & Allocation)
*   **Layout 计算**: 使用 `Layout::array::<T>(capacity)` 动态计算所需内存大小及对齐要求，防止整数溢出。
*   **手动分配**: 调用 `allocator.allocate(layout)` 获取裸指针。
*   **RAII 封装**: 实现 `Drop` trait。当 `DynamicArray` 离开作用域时：
    1.  调用 `std::ptr::drop_in_place` 析构所有有效元素 `[0..len]`。
    2.  调用 `allocator.deallocate` 释放堆内存块。

### 2.3 零成本抽象 (Zero-Cost Abstractions)
*   **Deref Coercion**: 实现 `Deref<Target=[T]>`，使动态数组自动退化为切片（Slice），复用 Rust 核心库的所有切片方法（如 `iter`, `sort`, `chunks`）。
*   **内联优化**: 关键路径方法（`push`, `pop`, `get`）标记 `#[inline]`，允许跨 crate 内联。

### 2.4 异常安全 (Exception Safety)
*   **Leak Amplification**: 在执行可能 Panic 的操作（如元素构造、克隆）前，先不修改 `len`，只有操作成功后才增加 `len`。
*   **SetLenOnDrop**: 使用 RAII 守卫（Guard）模式，在复杂操作（如 `drain` 或 `insert`）中，确保即使发生 Panic，也能正确设置 `len` 以便安全析构。

## 3. 所需库与工具

| 类别 | 库/工具名称 | 用途与集成方式 |
| :--- | :--- | :--- |
| **核心库** | `core::alloc::Layout` | 计算内存布局，处理对齐与溢出检查。 |
| **核心库** | `core::ptr::NonNull` | 提供协变的非空指针包装，优化 `Option<DynamicArray>` 大小。 |
| **核心库** | `core::mem::MaybeUninit` | 处理未初始化内存，避免未定义行为（UB）。 |
| **Alloc库** | `alloc::alloc::{alloc, dealloc}` | 底层内存分配与释放 API。 |
| **检测工具** | **Miri** | Rust 解释器，专门用于检测 Unsafe 代码中的 UB（未定义行为）、对齐错误和内存泄漏。 |
| **检测工具** | **Valgrind / Massif** | 运行时内存分析，检测堆内存使用峰值与泄漏（针对 FFI 场景）。 |
| **检测工具** | **Cargo Geiger** | 统计项目中 `unsafe` 代码块的使用情况，用于审计。 |

## 4. 设计约束与规范

### 4.1 内存安全规则 (Unsafe Contract)
*   **Invariant 1**: `ptr` 必须始终指向由 `alloc` 分配的有效内存块（除非 `cap == 0`）。
*   **Invariant 2**: `len <= cap` 必须始终成立。
*   **Invariant 3**: 索引 `0..len` 范围内的内存必须已初始化。
*   **Invariant 4**: `cap` 不能超过 `isize::MAX` 字节，以兼容 LLVM GEP 指令限制。

### 4.2 错误处理
*   **OOM**: 提供 `try_push` / `try_reserve` 返回 `Result<(), AllocError>`，允许调用者处理内存耗尽。
*   **IndexOutOfBounds**: `get(index)` 返回 `Option<&T>`，`index` 运算符直接 Panic。

### 4.3 代码标准
*   所有 `unsafe` 块必须在上方注释 `// SAFETY: ...`，详细说明为何该操作是安全的。
*   单元测试覆盖率需达到 90% 以上，重点覆盖扩容、缩容、零大小类型（ZST）和 Panic 恢复场景。

## 5. 验证与测试方案

### 5.1 内存泄漏检测
编写专门的 `LeakTest`，使用原子计数器（AtomicCounter）追踪对象构造与析构次数，确保 `Constructed == Dropped`。

```rust
#[test]
fn test_memory_leak() {
    let allocated = AtomicUsize::new(0);
    {
        let vec = DynamicArray::new();
        vec.push(TrackedObject::new(&allocated));
        // Scope ends, drop called
    }
    assert_eq!(allocated.load(Ordering::SeqCst), 0);
}
```

### 5.2 Miri 形式化验证
在 CI 流程中集成 Miri：
`cargo +nightly miri test`
Miri 将捕获所有非法的内存访问、未对齐指针使用和重叠内存拷贝。

### 5.3 压力与模糊测试 (Fuzzing)
使用 `arbitrary` 和 `libfuzzer-sys` 生成随机的操作序列（Push, Pop, Insert, Remove, Shrink），长时间运行以发现潜在的逻辑漏洞和内存损坏。

---

**附注**：本方案的设计深受 Rust 标准库 `Vec` 实现的启发，但在错误处理和分配器接口上提供了更灵活的定制空间，适合作为编译器后端或运行时的基础组件。
