// Phase 1.8: 性能基准 — 编译 100 行 Trust 代码冷启动时间
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::process::Command;

fn bench_compile_hello(c: &mut Criterion) {
    c.bench_function("compile_hello", |b| {
        b.iter(|| {
            let output = Command::new("target/release/trustc")
                .args(["compile", "benches/inputs/hello.trust", "-o", "/tmp/trust_bench_hello"])
                .env("FERRO_RT_LIB", "target/release")
                .output()
                .expect("trustc should run");
            black_box(output);
        });
    });
}

criterion_group!(benches, bench_compile_hello);
criterion_main!(benches);
