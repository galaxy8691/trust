//! ts2rs 生成代码可选运行时：内建与后续扩展入口。

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
