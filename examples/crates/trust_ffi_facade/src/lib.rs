//! **FFI（C）与 Trust**：`extern "C"` 与 `unsafe` 调用留在本 crate；TS 只绑定到 **Rust 固有方法**
//! （`Cffi::add_nums`），不直接声明 C 原型。

unsafe extern "C" {
    fn trust_example_c_add(a: i32, b: i32) -> i32;
}

/// TS：`import { Cffi } from "trust_ffi_facade"`。
pub struct Cffi;

impl Cffi {
    /// 与 `OrmFacade::new` 相同：Trust 要求 `new` 带一个 `string` 实参。
    pub fn new(_unused: &String) -> Self {
        Self
    }

    /// 包装 C 函数；参数/返回均为 Trust 支持的 `number`（Rust 侧 `f64`）。
    pub fn add_nums(&self, a: f64, b: f64) -> f64 {
        unsafe { trust_example_c_add(a as i32, b as i32) as f64 }
    }
}
