//! ts2rs 生成代码**可选**运行时：内建与后续扩展入口。
//!
//! **当前状态**：`ts2rs` 生成的 Rust 默认**不**依赖本 crate（`ts2rs-driver` 临时 crate 无 `[dependencies]`）。
//! 此处 API 供将来接入或手工在生成代码中引用；字符串长度、`Math` 等已在 `ts2rs-hir` codegen 中内联实现。

use std::io::{self, BufRead};

/// 与 `console.log` 类似的单行输出（多参数以空格拼接显示）。
pub fn log_parts(parts: &[String]) {
    for (i, p) in parts.iter().enumerate() {
        if i > 0 {
            print!(" ");
        }
        print!("{p}");
    }
    println!();
}

/// 从标准输入读取一行（不含换行符）。占位 API：**生成器尚未发出对此函数的调用**。
pub fn read_stdin_line() -> io::Result<String> {
    let stdin = io::stdin();
    let mut line = String::new();
    stdin.lock().read_line(&mut line)?;
    if line.ends_with('\n') {
        line.pop();
        if line.ends_with('\r') {
            line.pop();
        }
    }
    Ok(line)
}
