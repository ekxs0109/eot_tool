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

    let Ok(reloaded) = load_sfnt(&serialized) else {
        panic!("serialized sfnt should remain parseable");
    };

    assert_eq!(parsed.version_tag(), reloaded.version_tag());
    assert_eq!(owned.version_tag(), reloaded.version_tag());
    assert_eq!(owned.tables().len(), reloaded.tables().len());

    for table in owned.tables() {
        let Some(reloaded_table) = reloaded.table(table.tag) else {
            panic!("serialized sfnt should contain table {:08x}", table.tag);
        };
        assert_eq!(table.data, reloaded_table.data);
    }
});
