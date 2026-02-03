use std::alloc::{alloc, dealloc, realloc, Layout, handle_alloc_error};
use std::ptr::{self, NonNull};
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::mem;

#[derive(Debug, Clone, Copy)]
pub struct AllocError;

impl std::fmt::Display for AllocError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Allocation failed")
    }
}

impl std::error::Error for AllocError {}

/// 基于 RAII 的动态数组实现
/// 遵循 RUST_RAII_MEMORY_DESIGN.md 规范
pub struct DynamicArray<T> {
    ptr: NonNull<T>,
    cap: usize,
    len: usize,
    _marker: PhantomData<T>,
}

// SAFETY: 只要 T 是 Send，DynamicArray<T> 就可以跨线程转移所有权
unsafe impl<T: Send> Send for DynamicArray<T> {}
// SAFETY: 只要 T 是 Sync，DynamicArray<T> 就可以在多线程间共享引用
unsafe impl<T: Sync> Sync for DynamicArray<T> {}

impl<T> DynamicArray<T> {
    /// 创建一个空的动态数组，不分配内存
    pub fn new() -> Self {
        // 不支持零尺寸类型（ZST），简化演示逻辑
        assert!(mem::size_of::<T>() != 0, "Zero-sized types not supported in this demo");
        Self {
            ptr: NonNull::dangling(),
            cap: 0,
            len: 0,
            _marker: PhantomData,
        }
    }

    /// 创建具有指定初始容量的动态数组
    pub fn with_capacity(capacity: usize) -> Self {
        assert!(mem::size_of::<T>() != 0, "Zero-sized types not supported in this demo");
        if capacity == 0 {
            return Self::new();
        }

        let layout = Layout::array::<T>(capacity).expect("Capacity overflow");
        assert!(layout.size() <= isize::MAX as usize, "Allocation too large");

        let ptr = unsafe { alloc(layout) };
        let ptr = match NonNull::new(ptr as *mut T) {
            Some(p) => p,
            None => handle_alloc_error(layout),
        };

        Self {
            ptr,
            cap: capacity,
            len: 0,
            _marker: PhantomData,
        }
    }

    /// 获取当前元素数量
    pub fn len(&self) -> usize {
        self.len
    }

    /// 获取当前容量
    pub fn capacity(&self) -> usize {
        self.cap
    }

    /// 在末尾添加元素
    pub fn push(&mut self, elem: T) {
        if self.len == self.cap {
            self.grow();
        }

        unsafe {
            // SAFETY: 我们已确保有足够的容量，且指针有效
            ptr::write(self.ptr.as_ptr().add(self.len), elem);
            // 异常安全：只有在写入成功后才增加 len
            self.len += 1;
        }
    }

    /// 弹出末尾元素
    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            None
        } else {
            self.len -= 1;
            unsafe {
                // SAFETY: len 已减 1，该位置是有效的已初始化元素
                Some(ptr::read(self.ptr.as_ptr().add(self.len)))
            }
        }
    }

    /// 在指定位置插入元素
    pub fn insert(&mut self, index: usize, elem: T) {
        assert!(index <= self.len, "Index out of bounds");
        if self.len == self.cap {
            self.grow();
        }

        unsafe {
            let p = self.ptr.as_ptr().add(index);
            // 将 index 之后的元素向后移动一位
            ptr::copy(p, p.add(1), self.len - index);
            ptr::write(p, elem);
            self.len += 1;
        }
    }

    /// 移除并返回指定位置的元素
    pub fn remove(&mut self, index: usize) -> T {
        assert!(index < self.len, "Index out of bounds");
        unsafe {
            self.len -= 1;
            let p = self.ptr.as_ptr().add(index);
            let result = ptr::read(p);
            // 将 index 之后的元素向前移动一位
            ptr::copy(p.add(1), p, self.len - index);
            result
        }
    }

    /// 尝试预留容量（OOM 防护接口）
    pub fn try_reserve(&mut self, additional: usize) -> Result<(), AllocError> {
        let required_cap = self.len.checked_add(additional).ok_or(AllocError)?;
        if required_cap > self.cap {
            self.do_realloc(required_cap)?;
        }
        Ok(())
    }

    /// 缩小容量以适应当前长度
    pub fn shrink_to_fit(&mut self) {
        if self.len < self.cap {
            if self.len == 0 {
                // 如果长度为 0，释放所有内存
                let old_layout = Layout::array::<T>(self.cap).unwrap();
                unsafe {
                    dealloc(self.ptr.as_ptr() as *mut u8, old_layout);
                }
                self.ptr = NonNull::dangling();
                self.cap = 0;
            } else {
                let _ = self.do_realloc(self.len);
            }
        }
    }

    fn grow(&mut self) {
        let new_cap = if self.cap == 0 { 1 } else { self.cap * 2 };
        self.do_realloc(new_cap).expect("Allocation failed");
    }

    fn do_realloc(&mut self, new_cap: usize) -> Result<(), AllocError> {
        let new_layout = Layout::array::<T>(new_cap).map_err(|_| AllocError)?;
        assert!(new_layout.size() <= isize::MAX as usize, "Allocation too large");

        let new_ptr = if self.cap == 0 {
            unsafe { alloc(new_layout) }
        } else {
            let old_layout = Layout::array::<T>(self.cap).unwrap();
            unsafe {
                realloc(self.ptr.as_ptr() as *mut u8, old_layout, new_layout.size())
            }
        };

        self.ptr = match NonNull::new(new_ptr as *mut T) {
            Some(p) => p,
            None => return Err(AllocError),
        };
        self.cap = new_cap;
        Ok(())
    }
}

impl<T> Drop for DynamicArray<T> {
    fn drop(&mut self) {
        if self.cap != 0 {
            unsafe {
                // 1. 析构所有有效元素
                ptr::drop_in_place(ptr::slice_from_raw_parts_mut(self.ptr.as_ptr(), self.len));
                // 2. 释放内存块
                let layout = Layout::array::<T>(self.cap).unwrap();
                dealloc(self.ptr.as_ptr() as *mut u8, layout);
            }
        }
    }
}

impl<T> Deref for DynamicArray<T> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        unsafe { std::slice::from_raw_parts(self.ptr.as_ptr(), self.len) }
    }
}

impl<T> DerefMut for DynamicArray<T> {
    fn deref_mut(&mut self) -> &mut [T] {
        unsafe { std::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len) }
    }
}

impl<T> Default for DynamicArray<T> {
    fn default() -> Self {
        Self::new()
    }
}

// 迭代器支持
pub struct IntoIter<T> {
    ptr: NonNull<T>,
    cap: usize,
    start: *const T,
    end: *const T,
    _marker: PhantomData<T>,
}

impl<T> IntoIterator for DynamicArray<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        let ptr = self.ptr;
        let cap = self.cap;
        let len = self.len;
        
        // 关键：避免 DynamicArray 的 Drop 被调用
        mem::forget(self);

        let start = ptr.as_ptr();
        let end = unsafe { start.add(len) };
        
        IntoIter {
            ptr,
            cap,
            start,
            end,
            _marker: PhantomData,
        }
    }
}

impl<T> Iterator for IntoIter<T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        if self.start == self.end {
            None
        } else {
            unsafe {
                let result = ptr::read(self.start);
                self.start = self.start.add(1);
                Some(result)
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = (self.end as usize - self.start as usize) / mem::size_of::<T>();
        (len, Some(len))
    }
}

impl<T> Drop for IntoIter<T> {
    fn drop(&mut self) {
        if self.cap != 0 {
            unsafe {
                // 1. 析构剩余未消费的元素
                let remaining_len = (self.end as usize - self.start as usize) / mem::size_of::<T>();
                ptr::drop_in_place(ptr::slice_from_raw_parts_mut(self.start as *mut T, remaining_len));
                
                // 2. 释放内存块
                let layout = Layout::array::<T>(self.cap).unwrap();
                dealloc(self.ptr.as_ptr() as *mut u8, layout);
            }
        }
    }
}

// 借用迭代器
impl<'a, T> IntoIterator for &'a DynamicArray<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T> IntoIterator for &'a mut DynamicArray<T> {
    type Item = &'a mut T;
    type IntoIter = std::slice::IterMut<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<T> DynamicArray<T> {
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.deref().iter()
    }

    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, T> {
        self.deref_mut().iter_mut()
    }
}

#[cfg(test)]
mod tests;
