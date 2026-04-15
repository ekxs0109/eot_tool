#![no_main]

use fonttool_cff::inspect_otf_font;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = inspect_otf_font(data);
});
