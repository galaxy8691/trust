// Phase 1.8: 性能基准 — 编译 hello.trust 冷启动时间
// 用法: cargo bench --bench compile_bench
use std::process::Command;
use std::time::Instant;

fn main() {
    // 定位 workspace 根目录
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .unwrap_or(std::path::Path::new("."));
    let trustc_bin = workspace_root.join("target/release/trustc");
    let input_file = workspace_root.join("benches/inputs/hello.trust");

    let start = Instant::now();
    let output = Command::new(&trustc_bin)
        .args(["compile", input_file.to_str().unwrap(), "-o", "/tmp/trust_bench_hello"])
        .env("FERRO_RT_LIB", workspace_root.join("target/release"))
        .output()
        .expect("trustc should run");
    let elapsed = start.elapsed();

    if output.status.success() {
        println!("compile_hello: {:?}", elapsed);
    } else {
        eprintln!("compile failed: {}", String::from_utf8_lossy(&output.stderr));
        std::process::exit(1);
    }
}
