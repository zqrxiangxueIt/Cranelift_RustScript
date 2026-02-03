use criterion::{criterion_group, criterion_main, Criterion, black_box};
use memory_bench::{create_linked_list, create_vec_allocation};

fn criterion_benchmark(c: &mut Criterion) {
    // 使用 System Allocator (默认)
    let mut group = c.benchmark_group("System Allocator");
    
    group.bench_function("linked_list_10k", |b| b.iter(|| {
        create_linked_list(black_box(10_000))
    }));

    group.bench_function("vec_1m_integers", |b| b.iter(|| {
        create_vec_allocation(black_box(1_000_000))
    }));
    
    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
