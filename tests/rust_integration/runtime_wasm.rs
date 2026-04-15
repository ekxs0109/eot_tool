use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};

use fonttool_runtime::{
    convert_otf_to_embedded_font, default_runtime_diagnostics, resolve_runtime_diagnostics,
    run_indexed_tasks, runtime_thread_mode, ConvertRequest, IndexedTaskFailure, OutputKind,
    RequestedRuntimeMode, RuntimeError, RuntimeSchedulingOptions,
};
use fonttool_wasm::{
    wasm_convert_otf_to_embedded_font, wasm_runtime_get_diagnostics, wasm_runtime_thread_mode,
};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root should exist")
}

fn fixture(path: &str) -> PathBuf {
    workspace_root().join(path)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProbeError {
    CorruptData,
    InvalidPadding,
}

fn runtime_probe(
    task_count: usize,
    thread_override: Option<&str>,
    requested_mode: RequestedRuntimeMode,
) -> fonttool_runtime::RuntimeDiagnostics {
    resolve_runtime_diagnostics(
        task_count,
        RuntimeSchedulingOptions {
            thread_override,
            requested_mode,
        },
    )
}

fn assert_runtime_error(
    result: Result<fonttool_runtime::RuntimeDiagnostics, IndexedTaskFailure<ProbeError>>,
) -> IndexedTaskFailure<ProbeError> {
    result.expect_err("runtime run should fail")
}

#[test]
fn runtime_mode_matches_supported_strings() {
    assert!(matches!(
        runtime_thread_mode().as_str(),
        "single-thread" | "pthreads"
    ));
    assert!(matches!(
        wasm_runtime_thread_mode().as_str(),
        "single-thread" | "pthreads"
    ));
}

#[test]
fn default_runtime_diagnostics_keep_preferred_mode_for_idle_probe() {
    const CHILD_ENV: &str = "FONTTOOL_IDLE_DIAGNOSTICS_CHILD";

    if std::env::var_os(CHILD_ENV).is_some() {
        let diagnostics = default_runtime_diagnostics();
        let expected_mode = match runtime_thread_mode() {
            fonttool_runtime::RuntimeThreadMode::SingleThread => "single",
            fonttool_runtime::RuntimeThreadMode::Pthreads => "threaded",
        };

        assert_eq!(diagnostics.effective_threads, 0);
        assert_eq!(diagnostics.resolved_mode, expected_mode);
        assert_eq!(diagnostics.fallback_reason, None);
        return;
    }

    if runtime_thread_mode() == fonttool_runtime::RuntimeThreadMode::SingleThread {
        return;
    }

    let output = Command::new(std::env::current_exe().expect("test binary should exist"))
        .arg("--exact")
        .arg("default_runtime_diagnostics_keep_preferred_mode_for_idle_probe")
        .env(CHILD_ENV, "1")
        .env("EOT_TOOL_THREADS", "1")
        .output()
        .expect("child test run should execute");

    assert!(
        output.status.success(),
        "child stdout:\n{}\nchild stderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn runtime_and_wasm_default_diagnostics_stay_in_sync() {
    let runtime = default_runtime_diagnostics();
    let wasm = wasm_runtime_get_diagnostics();
    let expected_mode = match runtime_thread_mode() {
        fonttool_runtime::RuntimeThreadMode::SingleThread => "single",
        fonttool_runtime::RuntimeThreadMode::Pthreads => "threaded",
    };

    assert_eq!(runtime, wasm);
    assert!(runtime.requested_threads >= 1);
    assert_eq!(runtime.effective_threads, 0);
    assert_eq!(runtime.resolved_mode, expected_mode);
    assert_eq!(runtime.fallback_reason, None);
}

#[test]
fn runtime_reports_requested_and_effective_threads_after_task_clamp() {
    let diagnostics = runtime_probe(3, Some("8"), RequestedRuntimeMode::Auto);

    assert_eq!(diagnostics.requested_threads, 8);
    assert_eq!(diagnostics.effective_threads, 3);
    assert_eq!(diagnostics.resolved_mode, "threaded");
    assert_eq!(
        diagnostics.fallback_reason.as_deref(),
        Some("task-count-clamped")
    );
}

#[test]
fn runtime_invalid_override_falls_back_to_default_resolution() {
    let fallback = runtime_probe(2, None, RequestedRuntimeMode::Auto);
    let diagnostics = runtime_probe(2, Some("invalid"), RequestedRuntimeMode::Auto);

    assert_eq!(diagnostics, fallback);
}

#[test]
fn runtime_requested_mode_transitions_between_single_and_threaded() {
    let single = runtime_probe(4, Some("8"), RequestedRuntimeMode::Single);
    assert_eq!(single.requested_threads, 1);
    assert_eq!(single.effective_threads, 1);
    assert_eq!(single.resolved_mode, "single");
    assert_eq!(single.fallback_reason.as_deref(), Some("requested-single"));

    let threaded = runtime_probe(4, Some("8"), RequestedRuntimeMode::Threaded);
    assert_eq!(threaded.requested_threads, 8);
    assert_eq!(threaded.effective_threads, 4);
    assert_eq!(threaded.resolved_mode, "threaded");
    assert_eq!(
        threaded.fallback_reason.as_deref(),
        Some("task-count-clamped")
    );
}

#[test]
fn runtime_waits_for_all_started_tasks_before_reporting_error() {
    let seen: Arc<Vec<AtomicUsize>> =
        Arc::new((0..4).map(|_| AtomicUsize::new(0)).collect::<Vec<_>>());
    let task_seen = Arc::clone(&seen);

    let error = assert_runtime_error(run_indexed_tasks(
        4,
        RuntimeSchedulingOptions {
            thread_override: Some("4"),
            requested_mode: RequestedRuntimeMode::Auto,
        },
        move |index| {
            task_seen[index].store(1, Ordering::Relaxed);
            if index == 2 {
                Err(ProbeError::CorruptData)
            } else {
                Ok(())
            }
        },
    ));

    assert_eq!(error.index, 2);
    assert_eq!(error.error, ProbeError::CorruptData);
    for entry in &*seen {
        assert_eq!(entry.load(Ordering::Relaxed), 1);
    }
}

#[test]
fn runtime_uses_lowest_failing_index_regardless_of_completion_order() {
    let options = RuntimeSchedulingOptions {
        thread_override: Some("4"),
        requested_mode: RequestedRuntimeMode::Auto,
    };
    let seen: Arc<Vec<AtomicUsize>> =
        Arc::new((0..4).map(|_| AtomicUsize::new(0)).collect::<Vec<_>>());
    let allow_index_zero = Arc::new(AtomicBool::new(false));
    let task_seen = Arc::clone(&seen);
    let release_zero = Arc::clone(&allow_index_zero);
    let diagnostics = runtime_probe(4, options.thread_override, options.requested_mode);

    let error = assert_runtime_error(run_indexed_tasks(4, options, move |index| {
        task_seen[index].store(1, Ordering::Relaxed);

        if index == 0 {
            if diagnostics.effective_threads > 1 {
                while !release_zero.load(Ordering::Acquire) {
                    std::hint::spin_loop();
                }
            }
            return Err(ProbeError::CorruptData);
        }

        if index == 3 {
            release_zero.store(true, Ordering::Release);
            return Err(ProbeError::InvalidPadding);
        }

        Ok(())
    }));

    assert_eq!(error.index, 0);
    assert_eq!(error.error, ProbeError::CorruptData);
    assert_eq!(seen[0].load(Ordering::Relaxed), 1);
    assert_eq!(seen[3].load(Ordering::Relaxed), 1);
}

#[test]
fn runtime_bridge_rejects_static_cff_conversion_until_phase3() {
    let error = convert_otf_to_embedded_font(ConvertRequest {
        input_path: &fixture("testdata/cff-static.otf"),
        output_kind: OutputKind::Eot,
        variation_axes: None,
    })
    .expect_err("static CFF conversion should stay outside the Phase 1 runtime boundary");

    assert_eq!(
        error,
        RuntimeError::Cff(fonttool_runtime::CffError::EncodeDeferredToPhase3)
    );
}

#[test]
fn runtime_bridge_rejects_fntdata_output_until_phase2() {
    let error = convert_otf_to_embedded_font(ConvertRequest {
        input_path: &fixture("testdata/cff-static.otf"),
        output_kind: OutputKind::Fntdata,
        variation_axes: None,
    })
    .expect_err(".fntdata output should stay outside the Phase 1 runtime boundary");

    assert_eq!(
        error,
        RuntimeError::Backend(
            "PowerPoint-compatible .fntdata output remains Phase 2-owned; use the archived native binary for compatibility flows".to_string()
        )
    );
}

#[test]
fn wasm_bridge_rejects_fntdata_output_until_phase2() {
    let error = wasm_convert_otf_to_embedded_font(ConvertRequest {
        input_path: &fixture("testdata/cff-static.otf"),
        output_kind: OutputKind::Fntdata,
        variation_axes: None,
    })
    .expect_err(".fntdata output should stay outside the Phase 1 wasm boundary");

    assert_eq!(
        error,
        RuntimeError::Backend(
            "PowerPoint-compatible .fntdata output remains Phase 2-owned; use the archived native binary for compatibility flows".to_string()
        )
    );
}

#[test]
fn runtime_bridge_rejects_variation_axes_for_static_input() {
    let error = convert_otf_to_embedded_font(ConvertRequest {
        input_path: &fixture("testdata/cff-static.otf"),
        output_kind: OutputKind::Eot,
        variation_axes: Some("wght=700"),
    })
    .expect_err("static cff input should reject variation request");

    assert_eq!(
        error,
        RuntimeError::Cff(fonttool_runtime::CffError::VariationRejectedForStaticInput)
    );
}

#[test]
fn wasm_bridge_surfaces_runtime_validation_errors() {
    let error = wasm_convert_otf_to_embedded_font(ConvertRequest {
        input_path: &fixture("testdata/cff-static.otf"),
        output_kind: OutputKind::Eot,
        variation_axes: Some("wght=700"),
    })
    .expect_err("wasm bridge should surface runtime validation errors");

    assert!(matches!(
        error,
        RuntimeError::Cff(fonttool_runtime::CffError::VariationRejectedForStaticInput)
    ));
}

#[test]
fn runtime_bridge_rejects_variable_font_conversion_for_now() {
    let error = convert_otf_to_embedded_font(ConvertRequest {
        input_path: &fixture("testdata/cff2-variable.otf"),
        output_kind: OutputKind::Eot,
        variation_axes: Some("wght=700"),
    })
    .expect_err("variable conversion should stay explicitly unsupported");

    assert_eq!(
        error,
        RuntimeError::Backend(
            "runtime bridge does not yet support variable-font conversion".to_string()
        )
    );
}

#[test]
fn wasm_bridge_rejects_variable_font_conversion_for_now() {
    let error = wasm_convert_otf_to_embedded_font(ConvertRequest {
        input_path: &fixture("testdata/cff2-variable.otf"),
        output_kind: OutputKind::Eot,
        variation_axes: Some("wght=700"),
    })
    .expect_err("variable conversion should stay explicitly unsupported");

    assert_eq!(
        error,
        RuntimeError::Backend(
            "runtime bridge does not yet support variable-font conversion".to_string()
        )
    );
}
