#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Phase 1.8: TIR borrowck fuzz 骨架
    // 对随机输入构造最小 TIR 程序，验证 borrowck 不 panic。
    // Phase 2: 引入 Arbitrary 派生以构造有效 TIR 图。
    if data.len() < 4 {
        return;
    }
    let program = trust_tir::tir::TirProgram {
        file: String::new(),
        functions: vec![],
    };
    let _ = trust_tir::borrowck::check_borrows(&program);
    let _ = trust_tir::moveck::check_moves(&program);
});
