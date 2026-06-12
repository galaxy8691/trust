#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Phase 1.2: 将 data 作为 .trust 源码输入 trust_parser
    todo!("parser fuzz — Phase 1.2")
});
