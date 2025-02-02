#![no_main]

use git_config::parser::parse_from_bytes;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Don't name this _; Rust may optimize it out.
    let _a = parse_from_bytes(data);
});
