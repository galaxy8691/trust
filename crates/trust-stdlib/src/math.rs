//! `Math.*` 内建（`f64` 语义）。

pub fn abs(x: f64) -> f64 {
    x.abs()
}

pub fn min(a: f64, b: f64) -> f64 {
    a.min(b)
}

pub fn max(a: f64, b: f64) -> f64 {
    a.max(b)
}

pub fn floor(x: f64) -> f64 {
    x.floor()
}

pub fn ceil(x: f64) -> f64 {
    x.ceil()
}

pub fn trunc(x: f64) -> f64 {
    x.trunc()
}

pub fn round(x: f64) -> f64 {
    x.round()
}

pub fn sign(x: f64) -> f64 {
    x.signum()
}

pub fn pow(base: f64, exp: f64) -> f64 {
    if exp < 0.0 {
        panic!("Math.pow: negative exponent");
    }
    base.powf(exp)
}
