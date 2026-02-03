use criterion::{black_box, criterion_group, criterion_main, Criterion};
use raii_demo::DynamicArray;

fn bench_push_std(c: &mut Criterion) {
    c.bench_function("std_vec_push", |b| {
        b.iter(|| {
            let mut v = Vec::new();
            for i in 0..1000 {
                v.push(black_box(i));
            }
            v
        })
    });
}

fn bench_push_raii(c: &mut Criterion) {
    c.bench_function("raii_array_push", |b| {
        b.iter(|| {
            let mut v = DynamicArray::new();
            for i in 0..1000 {
                v.push(black_box(i));
            }
            v
        })
    });
}

fn bench_iter_std(c: &mut Criterion) {
    let v: Vec<i32> = (0..1000).collect();
    c.bench_function("std_vec_iter", |b| {
        b.iter(|| {
            let mut sum = 0;
            for &x in black_box(&v) {
                sum += x;
            }
            sum
        })
    });
}

fn bench_iter_raii(c: &mut Criterion) {
    let mut v = DynamicArray::new();
    for i in 0..1000 {
        v.push(i);
    }
    c.bench_function("raii_array_iter", |b| {
        b.iter(|| {
            let mut sum = 0;
            for &x in black_box(&v) {
                sum += x;
            }
            sum
        })
    });
}

criterion_group!(benches, bench_push_std, bench_push_raii, bench_iter_std, bench_iter_raii);
criterion_main!(benches);
