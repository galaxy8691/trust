#![no_main]
use libfuzzer_sys::fuzz_target;
use trust_parser::parser;

fuzz_target!(|data: &[u8]| {
    if let Ok(input) = std::str::from_utf8(data) {
        let _ = parser::parse(input);
    }
});
