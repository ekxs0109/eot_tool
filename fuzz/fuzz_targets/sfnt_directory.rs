#![no_main]

use fonttool_sfnt::{load_sfnt, parse_sfnt, serialize_sfnt};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(parsed) = parse_sfnt(data) else {
        return;
    };

    let Ok(owned) = load_sfnt(data) else {
        return;
    };

    let Ok(serialized) = serialize_sfnt(&owned) else {
        return;
    };

    let Ok(reparsed) = parse_sfnt(&serialized) else {
        panic!("serialized sfnt should remain parseable");
    };

    assert_eq!(parsed.version_tag(), reparsed.version_tag());
    assert_eq!(
        parsed.table_directory().len(),
        reparsed.table_directory().len()
    );
});
