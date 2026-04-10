//! `Number.parseInt` / `parseFloat`（结果为 `f64`）。

pub fn parse_int_trim(s: &str) -> f64 {
    let p = s.trim().parse::<i64>().unwrap_or(0);
    p as f64
}

pub fn parse_int_radix(s: &str, radix: u32) -> f64 {
    i64::from_str_radix(s.trim(), radix).unwrap_or(0) as f64
}

pub fn parse_float_trim(s: &str) -> f64 {
    s.trim().parse::<f64>().unwrap_or(0.0)
}
