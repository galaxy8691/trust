// §设计文档 §7.3: ferro_rt 运行时库接口映射
//!
//! 将 Trust 标准库调用映射到 ferro_rt crate 的对应函数。
//! Phase 1 仅支持 `console.log`。

/// §设计文档 §7.3: ferro_rt 运行时库路径 — 禁止硬编码字符串拼接
pub const FERRO_RT_CONSOLE: &str = "ferro_rt::console";
pub const FERRO_RT_CONSOLE_LOG: &str = "ferro_rt::console::log";

/// §7.3: 生成 `use ferro_rt::console;` 引入语句
pub fn emit_console_import() -> String {
    format!(
        "{use}{ferro_rt}{semi}\n",
        use = crate::codegen::USE_KEYWORD,
        ferro_rt = FERRO_RT_CONSOLE,
        semi = crate::codegen::SEMICOLON,
    )
}

/// §7.3: 生成 console.log 调用 — Phase 1 仅支持字符串参数
pub fn emit_console_log(msg: &str) -> String {
    format!(
        "{log}({quote}{msg}{quote})",
        log = FERRO_RT_CONSOLE_LOG,
        quote = "\"",
        msg = msg,
    )
}

/// §7.3: 生成 console.log 调用 — 表达式参数（通过 format! 转换）
pub fn emit_console_log_expr(expr: &str) -> String {
    format!(
        "{log}(&format!({quote}{{}}{quote}, {expr}))",
        log = FERRO_RT_CONSOLE_LOG,
        quote = "\"",
        expr = expr,
    )
}
