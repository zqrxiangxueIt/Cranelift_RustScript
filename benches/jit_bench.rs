use criterion::{black_box, criterion_group, criterion_main, Criterion};
use cranelift_jit_demo::jit::JIT;
use raii_demo::DynamicArray;

fn bench_math(c: &mut Criterion) {
    let mut jit = JIT::default();
    let code = r#"
    fn bench_sin(x: f64) -> (r: f64) {
        r = sin(x)
    }
    "#;
    let func_ptr = jit.compile(code).unwrap();
    let func: fn(f64) -> f64 = unsafe { std::mem::transmute(func_ptr) };

    c.bench_function("jit_sin", |b| b.iter(|| func(black_box(2.0))));
}

fn bench_native_sin(c: &mut Criterion) {
    c.bench_function("native_sin", |b| b.iter(|| black_box(2.0f64).sin()));
}

fn bench_array_sum(c: &mut Criterion) {
    let mut jit = JIT::default();
    let code = r#"
    fn bench_sum() -> (r: f64) {
        arr = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]
        r = toy_sum_array(arr)
    }
    "#;
    let func_ptr = jit.compile(code).unwrap();
    let func: fn() -> f64 = unsafe { std::mem::transmute(func_ptr) };
    
    c.bench_function("jit_sum_array", |b| b.iter(|| func()));
}

fn bench_dynamic_array(c: &mut Criterion) {
    let mut jit = JIT::default();
    let code = r#"
    fn bench_dynamic_array() -> (r: i64) {
        arr = array [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
        i = 0
        while i < 100 {
            array_push(arr, i)
            i = i + 1
        }
        r = array_len(arr)
    }
    "#;
    let func_ptr = jit.compile(code).unwrap();
    let func: fn() -> i64 = unsafe { std::mem::transmute(func_ptr) };
    
    c.bench_function("jit_dynamic_array", |b| b.iter(|| func()));
}

fn bench_native_dynamic_array(c: &mut Criterion) {
    c.bench_function("native_dynamic_array", |b| b.iter(|| {
        let mut arr = DynamicArray::<i64>::new();
        for i in 1..=10 { arr.push(i); }
        for i in 0..100 {
            arr.push(i as i64);
        }
        black_box(arr.len())
    }));
}

criterion_group!(benches, bench_math, bench_native_sin, bench_array_sum, bench_dynamic_array, bench_native_dynamic_array);
criterion_main!(benches);
