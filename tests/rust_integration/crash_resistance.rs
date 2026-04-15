use std::panic::{catch_unwind, AssertUnwindSafe};

use fonttool_cff::inspect_otf_font;
use fonttool_cff::CffError;
use fonttool_eot::{parse_eot_header, EotHeaderError};
use fonttool_mtx::parse_mtx_container;
use fonttool_mtx::MtxContainerError;
use fonttool_sfnt::{parse_sfnt, ParseError};

fn assert_no_panic<T, E>(label: &str, f: impl FnOnce() -> Result<T, E>) -> Result<T, E> {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(result) => result,
        Err(_) => panic!("{label} panicked on malformed input"),
    }
}

#[test]
fn malformed_font_input_returns_error_instead_of_panicking() {
    let err = assert_no_panic("parse_eot_header", || parse_eot_header(&[0xff; 32]))
        .expect_err("malformed EOT should fail");
    assert_eq!(err, EotHeaderError::Truncated);
}

#[test]
fn malformed_mtx_input_returns_truncated_error_instead_of_panicking() {
    let err = assert_no_panic("parse_mtx_container", || parse_mtx_container(&[0xff; 9]))
        .expect_err("malformed MTX should fail");
    assert_eq!(err, MtxContainerError::Truncated);
}

#[test]
fn malformed_sfnt_input_returns_truncated_error_instead_of_panicking() {
    let err = assert_no_panic("parse_sfnt", || parse_sfnt(&[0xff; 11]))
        .expect_err("malformed SFNT should fail");
    assert_eq!(err, ParseError::TruncatedHeader);
}

#[test]
fn malformed_cff_input_returns_wrapped_invalid_input_error_instead_of_panicking() {
    let err = assert_no_panic("inspect_otf_font", || inspect_otf_font(&[0xff; 32]))
        .expect_err("malformed CFF input should fail");

    match err {
        CffError::InvalidInput(message) => {
            assert!(
                message.starts_with("invalid SFNT: "),
                "expected wrapped SFNT error, got: {message}"
            );
        }
        other => panic!("expected invalid input error, got {other:?}"),
    }
}
