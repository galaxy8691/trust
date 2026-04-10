//! `console.log` / `error` / `debug`：空格拼接各行（与 `println!(\"{} {}\", …)` 一致）。

pub fn log_joined(to_stderr: bool, parts: Vec<String>) {
    if to_stderr {
        if parts.is_empty() {
            eprintln!();
            return;
        }
        let mut it = parts.iter();
        eprint!("{}", it.next().unwrap());
        for p in it {
            eprint!(" {}", p);
        }
        eprintln!();
    } else if parts.is_empty() {
        println!();
    } else {
        let mut it = parts.iter();
        print!("{}", it.next().unwrap());
        for p in it {
            print!(" {}", p);
        }
        println!();
    }
}
