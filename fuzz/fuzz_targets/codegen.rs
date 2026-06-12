#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Phase 1.5: 将 data 反序列化为随机 TIR 图，输入 codegen
    todo!("codegen fuzz — Phase 1.5")
});
