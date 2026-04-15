#![no_main]

use libfuzzer_sys::fuzz_target;
use fonttool_eot::parse_eot_header;

fuzz_target!(|data: &[u8]| {
    let _ = parse_eot_header(data);
});
