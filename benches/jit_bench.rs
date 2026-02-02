use criterion::{black_box, criterion_group, criterion_main, Criterion};
use cranelift_jit_demo::jit::JIT;

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
        r = sum_array(arr)
    }
    "#;
    let func_ptr = jit.compile(code).unwrap();
    let func: fn() -> f64 = unsafe { std::mem::transmute(func_ptr) };
    
    c.bench_function("jit_sum_array", |b| b.iter(|| func()));
}

criterion_group!(benches, bench_math, bench_native_sin, bench_array_sum);
criterion_main!(benches);
