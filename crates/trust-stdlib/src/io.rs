//! 同步 stdin / 文件与异步读文件。

pub fn read_stdin_line() -> String {
    use std::io::BufRead;
    let mut line = String::new();
    let _ = std::io::stdin().lock().read_line(&mut line);
    if line.ends_with('\n') {
        line.pop();
        if line.ends_with('\r') {
            line.pop();
        }
    }
    line
}

pub fn read_file_text(path: &str) -> String {
    std::fs::read_to_string(path).expect("readFileText failed")
}

#[cfg(feature = "async-io")]
pub async fn read_file_text_async(path: &str) -> String {
    tokio::fs::read_to_string(path)
        .await
        .expect("readFileTextAsync failed")
}
