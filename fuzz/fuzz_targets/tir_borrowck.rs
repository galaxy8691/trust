#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Phase 1.4: 将 data 反序列化为随机 TIR 图，输入 borrowck
    todo!("TIR borrowck fuzz — Phase 1.4")
});
