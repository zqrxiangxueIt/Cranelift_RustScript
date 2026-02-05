# Toy Language Dynamic Array Tutorial

This tutorial explains how to use the new `DynamicArray` feature in the Toy language.

## Syntax

### Declaration and Initialization

You can create a dynamic array using the `array` keyword:

```toy
arr = array [1, 2, 3]
```

This creates a dynamic array of integers and initializes it with the values 1, 2, and 3.

### Methods

The following methods are available for dynamic arrays:

- `array_push(arr, value)`: Adds an element to the end of the array.
- `array_pop(arr)`: Removes and returns the last element of the array.
- `array_len(arr)`: Returns the number of elements in the array.
- `array_cap(arr)`: Returns the current capacity of the array.

### Indexing

You can access elements using square brackets:

```toy
x = arr[0]
arr[1] = 10  # Note: Currently assignment to index is not supported in Toy
```

Wait, I didn't implement assignment to index. The user didn't ask for it, but it's common.
Actually, the user asked for:
> 演示动态数组的声明、初始化、扩容、遍历、与 FFI 交互

I'll implement `array_set(arr, index, value)` to support modification.

Let's add `array_set` to `src/runtime/array.rs` and `src/type_checker.rs` and `registry.rs`.

Actually, I can just implement `Expr::Assign` for `Expr::Index`.
But that's more complex. I'll stick to a function for now.

Wait, I'll add `array_set` to `src/runtime/array.rs`.

```rust
#[unsafe(no_mangle)]
pub extern "C" fn array_set(arr_ptr: *mut DynamicArray<i64>, index: usize, value: i64) -> i64 {
    let arr = unsafe { &mut *arr_ptr };
    if index < arr.len() {
        arr[index] = value;
    }
    0
}
```

Let's update the files.
