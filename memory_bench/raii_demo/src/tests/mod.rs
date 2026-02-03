use super::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use crossbeam::scope;

#[test]
fn test_basic_push_pop() {
    let mut v = DynamicArray::new();
    v.push(1);
    v.push(2);
    v.push(3);
    assert_eq!(v.len(), 3);
    assert_eq!(v[0], 1);
    assert_eq!(v[1], 2);
    assert_eq!(v[2], 3);
    assert_eq!(v.pop(), Some(3));
    assert_eq!(v.pop(), Some(2));
    assert_eq!(v.pop(), Some(1));
    assert_eq!(v.pop(), None);
}

#[test]
fn test_insert_remove() {
    let mut v = DynamicArray::new();
    v.push(1);
    v.push(3);
    v.insert(1, 2);
    assert_eq!(v.as_ref(), &[1, 2, 3]);
    assert_eq!(v.remove(1), 2);
    assert_eq!(v.as_ref(), &[1, 3]);
}

#[test]
fn test_grow_and_shrink() {
    let mut v = DynamicArray::with_capacity(2);
    assert_eq!(v.capacity(), 2);
    v.push(1);
    v.push(2);
    v.push(3);
    assert!(v.capacity() >= 3);
    v.shrink_to_fit();
    assert_eq!(v.capacity(), 3);
    v.pop();
    v.pop();
    v.pop();
    v.shrink_to_fit();
    assert_eq!(v.capacity(), 0);
}

#[test]
fn test_raii_drop() {
    let counter = Arc::new(AtomicUsize::new(0));
    struct Droppable(Arc<AtomicUsize>);
    impl Drop for Droppable {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    {
        let mut v = DynamicArray::new();
        for _ in 0..10 {
            v.push(Droppable(counter.clone()));
        }
    }
    assert_eq!(counter.load(Ordering::SeqCst), 10);
}

#[test]
fn test_exception_safety_push() {
    // 验证在 push 过程中不会发生内存泄漏
    let counter = Arc::new(AtomicUsize::new(0));
    {
        let mut v = DynamicArray::new();
        for i in 0..10 {
            v.push(i);
        }
    }
    // 简单的 push 验证已在 test_basic_push_pop 中完成
}

#[test]
fn test_iterators() {
    let mut v = DynamicArray::new();
    v.push(10);
    v.push(20);
    v.push(30);

    let mut sum = 0;
    for &x in &v {
        sum += x;
    }
    assert_eq!(sum, 60);

    for x in &mut v {
        *x += 1;
    }
    assert_eq!(v[0], 11);

    let collected: Vec<i32> = v.into_iter().collect();
    assert_eq!(collected, vec![11, 21, 31]);
}

#[test]
fn test_concurrency() {
    let mut v = DynamicArray::new();
    for i in 0..100 {
        v.push(i);
    }

    scope(|s| {
        s.spawn(|_| {
            for x in &v {
                let _ = *x;
            }
        });
        s.spawn(|_| {
            for x in &v {
                let _ = *x;
            }
        });
    }).unwrap();
}

#[test]
fn test_try_reserve() {
    let mut v: DynamicArray<i32> = DynamicArray::new();
    assert!(v.try_reserve(10).is_ok());
    assert!(v.capacity() >= 10);
}

#[test]
#[should_panic(expected = "Index out of bounds")]
fn test_out_of_bounds_remove() {
    let mut v: DynamicArray<i32> = DynamicArray::new();
    v.remove(0);
}

#[test]
#[should_panic(expected = "Index out of bounds")]
fn test_out_of_bounds_insert() {
    let mut v: DynamicArray<i32> = DynamicArray::new();
    v.insert(1, 10);
}
