/// Trust 的 `console.log(...)` 编译目标。
///
/// Phase 1: 直接映射到 `println!`。
/// TODO(Phase 2): 支持多参数格式化输出。
///
/// # Panics
///
/// 无。
pub fn log(msg: &str) {
    println!("{}", msg);
}
