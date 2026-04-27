use raii_demo::DynamicArray;
use std::ptr;

/// Create a new dynamic array of i64
#[unsafe(no_mangle)]
pub extern "C" fn dynamic_array_new_i64() -> *mut DynamicArray<i64> {
    let arr = Box::new(DynamicArray::<i64>::new());
    Box::into_raw(arr)
}

/// Push an element to the dynamic array
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dynamic_array_push_i64(
    arr_ptr: *mut DynamicArray<i64>,
    elem: i64,
) -> i64 {
    let arr = unsafe { &mut *arr_ptr };
    arr.push(elem);
    0
}

/// Pop an element from the dynamic array
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dynamic_array_pop_i64(arr_ptr: *mut DynamicArray<i64>) -> i64 {
    let arr = unsafe { &mut *arr_ptr };
    arr.pop().unwrap_or(0)
}

/// Get the length of the dynamic array
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dynamic_array_len_i64(arr_ptr: *const DynamicArray<i64>) -> usize {
    let arr = unsafe { &*arr_ptr };
    arr.len()
}

/// Get the capacity of the dynamic array
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dynamic_array_cap_i64(arr_ptr: *const DynamicArray<i64>) -> usize {
    let arr = unsafe { &*arr_ptr };
    arr.capacity()
}

/// Get a pointer to an element at index
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dynamic_array_get_ptr_i64(
    arr_ptr: *mut DynamicArray<i64>,
    index: usize,
) -> *mut i64 {
    let arr = unsafe { &mut *arr_ptr };
    if index >= arr.len() {
        ptr::null_mut()
    } else {
        unsafe { arr.as_mut_ptr().add(index) }
    }
}

/// Set an element at index
/// Returns 0 on success, -1 on out of bounds
#[unsafe(no_mangle)]
pub unsafe extern "C" fn array_set(
    arr_ptr: *mut DynamicArray<i64>,
    index: usize,
    value: i64,
) -> i64 {
    let arr = unsafe { &mut *arr_ptr };
    if index >= arr.len() {
        return -1; // Error: index out of bounds
    }
    arr[index] = value;
    0 // Success
}

/// Drop the dynamic array
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dynamic_array_drop_i64(arr_ptr: *mut DynamicArray<i64>) -> i64 {
    if !arr_ptr.is_null() {
        unsafe { let _ = Box::from_raw(arr_ptr); }
    }
    0
}

// ============================================================================
// F64 Dynamic Array Functions
// ============================================================================

/// Create a new dynamic array of f64
#[unsafe(no_mangle)]
pub extern "C" fn dynamic_array_new_f64() -> *mut DynamicArray<f64> {
    let arr = Box::new(DynamicArray::<f64>::new());
    Box::into_raw(arr)
}

/// Push an element to the dynamic array
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dynamic_array_push_f64(
    arr_ptr: *mut DynamicArray<f64>,
    elem: f64,
) -> i64 {
    let arr = unsafe { &mut *arr_ptr };
    arr.push(elem);
    0
}

/// Pop an element from the dynamic array
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dynamic_array_pop_f64(arr_ptr: *mut DynamicArray<f64>) -> f64 {
    let arr = unsafe { &mut *arr_ptr };
    arr.pop().unwrap_or(0.0)
}

/// Get the length of the dynamic array
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dynamic_array_len_f64(arr_ptr: *const DynamicArray<f64>) -> usize {
    let arr = unsafe { &*arr_ptr };
    arr.len()
}

/// Get the capacity of the dynamic array
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dynamic_array_cap_f64(arr_ptr: *const DynamicArray<f64>) -> usize {
    let arr = unsafe { &*arr_ptr };
    arr.capacity()
}

/// Get a pointer to an element at index
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dynamic_array_get_ptr_f64(
    arr_ptr: *mut DynamicArray<f64>,
    index: usize,
) -> *mut f64 {
    let arr = unsafe { &mut *arr_ptr };
    if index >= arr.len() {
        ptr::null_mut()
    } else {
        unsafe { arr.as_mut_ptr().add(index) }
    }
}

/// Set an element at index - Returns 0 on success, -1 on out of bounds
#[unsafe(no_mangle)]
pub unsafe extern "C" fn array_set_f64(
    arr_ptr: *mut DynamicArray<f64>,
    index: usize,
    value: f64,
) -> i64 {
    let arr = unsafe { &mut *arr_ptr };
    if index >= arr.len() {
        return -1; // Error: index out of bounds
    }
    arr[index] = value;
    0 // Success
}

/// Drop the dynamic array
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dynamic_array_drop_f64(arr_ptr: *mut DynamicArray<f64>) -> i64 {
    if !arr_ptr.is_null() {
        unsafe { let _ = Box::from_raw(arr_ptr); }
    }
    0
}

// ============================================================================
// Complex128 Dynamic Array Functions (stored as i128)
// ============================================================================

/// Create a new dynamic array of complex128 (stored as i128)
#[unsafe(no_mangle)]
pub extern "C" fn dynamic_array_new_complex128() -> *mut DynamicArray<i128> {
    let arr = Box::new(DynamicArray::<i128>::new());
    Box::into_raw(arr)
}

/// Push an element to the dynamic array
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dynamic_array_push_complex128(
    arr_ptr: *mut DynamicArray<i128>,
    elem: i128,
) -> i64 {
    let arr = unsafe { &mut *arr_ptr };
    arr.push(elem);
    0
}

/// Pop an element from the dynamic array
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dynamic_array_pop_complex128(arr_ptr: *mut DynamicArray<i128>) -> i128 {
    let arr = unsafe { &mut *arr_ptr };
    arr.pop().unwrap_or(0)
}

/// Get the length of the dynamic array
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dynamic_array_len_complex128(arr_ptr: *const DynamicArray<i128>) -> usize {
    let arr = unsafe { &*arr_ptr };
    arr.len()
}

/// Get the capacity of the dynamic array
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dynamic_array_cap_complex128(arr_ptr: *const DynamicArray<i128>) -> usize {
    let arr = unsafe { &*arr_ptr };
    arr.capacity()
}

/// Get a pointer to an element at index
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dynamic_array_get_ptr_complex128(
    arr_ptr: *mut DynamicArray<i128>,
    index: usize,
) -> *mut i128 {
    let arr = unsafe { &mut *arr_ptr };
    if index >= arr.len() {
        ptr::null_mut()
    } else {
        unsafe { arr.as_mut_ptr().add(index) }
    }
}

/// Set an element at index - Returns 0 on success, -1 on out of bounds
#[unsafe(no_mangle)]
pub unsafe extern "C" fn array_set_complex128(
    arr_ptr: *mut DynamicArray<i128>,
    index: usize,
    value: i128,
) -> i64 {
    let arr = unsafe { &mut *arr_ptr };
    if index >= arr.len() {
        return -1; // Error: index out of bounds
    }
    arr[index] = value;
    0 // Success
}

/// Drop the dynamic array
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dynamic_array_drop_complex128(arr_ptr: *mut DynamicArray<i128>) -> i64 {
    if !arr_ptr.is_null() {
        unsafe { let _ = Box::from_raw(arr_ptr); }
    }
    0
}
