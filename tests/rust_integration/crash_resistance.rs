use std::panic::{catch_unwind, AssertUnwindSafe};

use fonttool_cff::inspect_otf_font;
use fonttool_eot::parse_eot_header;
use fonttool_mtx::parse_mtx_container;
use fonttool_sfnt::parse_sfnt;

fn assert_no_panic<T, E>(label: &str, f: impl FnOnce() -> Result<T, E>) -> Result<T, E> {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(result) => result,
        Err(_) => panic!("{label} panicked on malformed input"),
    }
}

fn decode_bytes(bytes: &[u8]) -> Result<(), String> {
    let header = assert_no_panic("parse_eot_header", || parse_eot_header(bytes))
        .map_err(|error| format!("invalid EOT header: {error}"))?;

    let payload_start = header.header_length as usize;
    let payload_end = payload_start
        .checked_add(header.font_data_size as usize)
        .ok_or_else(|| "invalid EOT payload range".to_string())?;
    let payload = bytes
        .get(payload_start..payload_end)
        .ok_or_else(|| "invalid EOT payload range".to_string())?;

    let container = assert_no_panic("parse_mtx_container", || parse_mtx_container(payload))
        .map_err(|error| format!("invalid MTX container: {error}"))?;
    let sfnt_bytes = container.block1;
    let _ = assert_no_panic("parse_sfnt", || parse_sfnt(sfnt_bytes))
        .map_err(|error| format!("decoded SFNT is invalid: {error}"))?;

    Ok(())
}

#[test]
fn malformed_font_input_returns_error_instead_of_panicking() {
    let result = decode_bytes(&[0xff; 32]);
    assert!(result.is_err());
}

#[test]
fn malformed_boundaries_return_structured_errors() {
    let cases: &[&[u8]] = &[
        &[],
        &[0xff; 4],
        &[0xff; 10],
        &[0xff; 32],
        &[0x00; 82],
    ];

    for bytes in cases {
        let eot = assert_no_panic("parse_eot_header", || parse_eot_header(bytes));
        assert!(eot.is_err(), "malformed EOT input should be rejected");

        let mtx = assert_no_panic("parse_mtx_container", || parse_mtx_container(bytes));
        assert!(mtx.is_err(), "malformed MTX input should be rejected");

        let sfnt = assert_no_panic("parse_sfnt", || parse_sfnt(bytes));
        assert!(sfnt.is_err(), "malformed SFNT input should be rejected");

        let cff = assert_no_panic("inspect_otf_font", || inspect_otf_font(bytes));
        assert!(cff.is_err(), "malformed CFF input should be rejected");
    }
}
