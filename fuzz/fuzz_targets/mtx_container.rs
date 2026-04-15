#![no_main]

use fonttool_mtx::parse_mtx_container;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = parse_mtx_container(data);
});
