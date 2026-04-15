//! Runtime-facing abstractions shared by Rust-native and WASM entry points.

use core::fmt;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Mutex,
};
use std::thread;

pub use fonttool_cff::CffError;

use fonttool_cff::{encode_otf_with_legacy_backend, inspect_otf_font};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeThreadMode {
    SingleThread,
    Pthreads,
}

impl RuntimeThreadMode {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            RuntimeThreadMode::SingleThread => "single-thread",
            RuntimeThreadMode::Pthreads => "pthreads",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeDiagnostics {
    pub requested_threads: usize,
    pub effective_threads: usize,
    // For idle/default diagnostics this reports the preferred execution mode the
    // runtime would use once work is scheduled.
    pub resolved_mode: String,
    pub fallback_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RequestedRuntimeMode {
    #[default]
    Auto,
    Single,
    Threaded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RuntimeSchedulingOptions<'a> {
    pub thread_override: Option<&'a str>,
    pub requested_mode: RequestedRuntimeMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedTaskFailure<E> {
    pub index: usize,
    pub error: E,
    pub diagnostics: RuntimeDiagnostics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputKind {
    Eot,
    Fntdata,
}

impl OutputKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            OutputKind::Eot => "eot",
            OutputKind::Fntdata => "fntdata",
        }
    }

    #[must_use]
    pub fn file_extension(self) -> &'static str {
        match self {
            OutputKind::Eot => "eot",
            OutputKind::Fntdata => "fntdata",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConvertRequest<'a> {
    pub input_path: &'a Path,
    pub output_kind: OutputKind,
    pub variation_axes: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConvertResult {
    pub data: Vec<u8>,
    pub diagnostics: RuntimeDiagnostics,
    pub output_kind: OutputKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeError {
    InvalidUtf8Path,
    Io(String),
    Backend(String),
    Cff(CffError),
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeError::InvalidUtf8Path => f.write_str("path must be valid utf-8"),
            RuntimeError::Io(message) => f.write_str(message),
            RuntimeError::Backend(message) => f.write_str(message),
            RuntimeError::Cff(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for RuntimeError {}

impl From<CffError> for RuntimeError {
    fn from(value: CffError) -> Self {
        RuntimeError::Cff(value)
    }
}

#[must_use]
pub fn runtime_thread_mode() -> RuntimeThreadMode {
    if runtime_supports_threads() {
        RuntimeThreadMode::Pthreads
    } else {
        RuntimeThreadMode::SingleThread
    }
}

#[must_use]
pub fn default_runtime_diagnostics() -> RuntimeDiagnostics {
    RuntimeDiagnostics {
        requested_threads: resolve_requested_threads(RuntimeSchedulingOptions::default()),
        effective_threads: 0,
        resolved_mode: match runtime_thread_mode() {
            RuntimeThreadMode::SingleThread => "single".to_string(),
            RuntimeThreadMode::Pthreads => "threaded".to_string(),
        },
        fallback_reason: None,
    }
}

#[must_use]
pub fn resolve_runtime_diagnostics(
    task_count: usize,
    options: RuntimeSchedulingOptions<'_>,
) -> RuntimeDiagnostics {
    let mut requested_threads = resolve_requested_threads(options);
    let mut effective_threads = requested_threads;
    let mut resolved_mode = "threaded";
    let mut fallback_reason = None;

    if options.requested_mode == RequestedRuntimeMode::Single {
        requested_threads = 1;
        effective_threads = 1;
        resolved_mode = "single";
        fallback_reason = Some("requested-single".to_string());
    } else if !runtime_supports_threads() {
        effective_threads = 1;
        resolved_mode = "single";
        if requested_threads > 1 || options.requested_mode == RequestedRuntimeMode::Threaded {
            fallback_reason = Some("threading-unavailable".to_string());
        }
    } else if requested_threads <= 1 {
        effective_threads = 1;
        resolved_mode = "single";
    }

    if task_count == 0 {
        effective_threads = 0;
    } else {
        if effective_threads > task_count {
            effective_threads = task_count;
            if effective_threads > 0
                && requested_threads > effective_threads
                && fallback_reason.is_none()
            {
                fallback_reason = Some("task-count-clamped".to_string());
            }
        }

        resolved_mode = if effective_threads <= 1 {
            "single"
        } else {
            "threaded"
        };
    }

    RuntimeDiagnostics {
        requested_threads,
        effective_threads,
        resolved_mode: resolved_mode.to_string(),
        fallback_reason,
    }
}

/// Runs every scheduled task to completion and aggregates errors by index.
///
/// This scheduler does not stop when a task fails. Once work has been claimed,
/// all claimed tasks are allowed to finish so the caller gets deterministic
/// diagnostics and the failure returned is always the lowest failing index.
pub fn run_indexed_tasks<E, F>(
    task_count: usize,
    options: RuntimeSchedulingOptions<'_>,
    task: F,
) -> Result<RuntimeDiagnostics, IndexedTaskFailure<E>>
where
    F: Fn(usize) -> Result<(), E> + Sync,
    E: Send,
{
    let diagnostics = resolve_runtime_diagnostics(task_count, options);
    if task_count == 0 {
        return Ok(diagnostics);
    }

    let effective_threads = diagnostics.effective_threads.max(1);
    let statuses = (0..task_count)
        .map(|_| Mutex::new(None))
        .collect::<Vec<Mutex<Option<E>>>>();
    let next_index = AtomicUsize::new(0);

    if effective_threads == 1 || !runtime_supports_threads() {
        drain_task_queue(&next_index, task_count, &task, &statuses);
    } else {
        thread::scope(|scope| {
            for _ in 1..effective_threads {
                let statuses = &statuses;
                let task = &task;
                let next_index = &next_index;
                scope.spawn(move || drain_task_queue(next_index, task_count, task, statuses));
            }
            drain_task_queue(&next_index, task_count, &task, &statuses);
        });
    }

    for (index, status) in statuses.into_iter().enumerate() {
        if let Some(error) = status
            .into_inner()
            .expect("task status mutex should not be poisoned")
        {
            return Err(IndexedTaskFailure {
                index,
                error,
                diagnostics,
            });
        }
    }

    Ok(diagnostics)
}

pub fn convert_otf_to_embedded_font(
    request: ConvertRequest<'_>,
) -> Result<ConvertResult, RuntimeError> {
    let temp_output = temp_runtime_output_path(request.output_kind);
    let diagnostics = run_indexed_tasks(1, RuntimeSchedulingOptions::default(), |_| {
        run_conversion_request(request, request.input_path, &temp_output)?;
        Ok::<(), RuntimeError>(())
    })
    .map_err(|failure| failure.error)?;

    let data = std::fs::read(&temp_output)
        .map_err(|error| RuntimeError::Io(format!("failed to read runtime output: {error}")))?;
    let _ = std::fs::remove_file(&temp_output);

    Ok(ConvertResult {
        data,
        diagnostics,
        output_kind: request.output_kind,
    })
}

#[cfg(test)]
fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root should exist")
}

fn run_conversion_request(
    request: ConvertRequest<'_>,
    input_path: &Path,
    output_path: &Path,
) -> Result<(), RuntimeError> {
    if let Some(variation_axes) = request.variation_axes {
        let font_bytes = std::fs::read(input_path)
            .map_err(|error| RuntimeError::Io(format!("failed to read OTF input: {error}")))?;
        let kind = inspect_otf_font(&font_bytes)?;
        if !kind.is_cff_flavor {
            return Err(RuntimeError::Backend(
                "runtime bridge expects OTF/CFF or OTF/CFF2 input".to_string(),
            ));
        }
        if !kind.is_variable {
            return Err(RuntimeError::Cff(CffError::VariationRejectedForStaticInput));
        }
        let _ = variation_axes;
        return Err(RuntimeError::Backend(
            "runtime bridge does not yet support variable-font conversion".to_string(),
        ));
    }

    encode_otf_with_legacy_backend(input_path, output_path).map_err(RuntimeError::from)
}

fn temp_runtime_output_path(output_kind: OutputKind) -> PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time should move forward")
        .as_nanos();

    std::env::temp_dir().join(format!(
        "fonttool-runtime-{}-{unique}.{}",
        std::process::id(),
        output_kind.file_extension()
    ))
}

fn runtime_supports_threads() -> bool {
    cfg!(not(target_arch = "wasm32")) || cfg!(target_feature = "atomics")
}

fn parse_threads_value(value: &str) -> Option<usize> {
    value.parse::<usize>().ok().filter(|threads| *threads > 0)
}

fn runtime_default_threads() -> usize {
    thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(1)
}

fn resolve_requested_threads(options: RuntimeSchedulingOptions<'_>) -> usize {
    let configured_threads = env::var("EOT_TOOL_THREADS").ok();
    options
        .thread_override
        .and_then(parse_threads_value)
        .or_else(|| configured_threads.as_deref().and_then(parse_threads_value))
        .unwrap_or_else(runtime_default_threads)
}

fn drain_task_queue<E, F>(
    next_index: &AtomicUsize,
    task_count: usize,
    task: &F,
    statuses: &[Mutex<Option<E>>],
) where
    F: Fn(usize) -> Result<(), E> + Sync,
    E: Send,
{
    loop {
        let index = next_index.fetch_add(1, Ordering::Relaxed);
        if index >= task_count {
            return;
        }

        if let Err(error) = task(index) {
            let mut slot = statuses[index]
                .lock()
                .expect("task status mutex should not be poisoned");
            *slot = Some(error);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    };

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum ProbeError {
        CorruptData,
        InvalidPadding,
    }

    fn fixture(path: &str) -> PathBuf {
        workspace_root().join(path)
    }

    fn assert_runtime_error(
        result: Result<RuntimeDiagnostics, IndexedTaskFailure<ProbeError>>,
    ) -> IndexedTaskFailure<ProbeError> {
        result.expect_err("runtime run should fail")
    }

    #[test]
    fn reports_runtime_mode_string() {
        assert!(matches!(
            runtime_thread_mode().as_str(),
            "single-thread" | "pthreads"
        ));
    }

    #[test]
    fn default_runtime_diagnostics_report_idle_preferred_mode() {
        let diagnostics = default_runtime_diagnostics();
        let expected_mode = match runtime_thread_mode() {
            RuntimeThreadMode::SingleThread => "single",
            RuntimeThreadMode::Pthreads => "threaded",
        };

        assert!(diagnostics.requested_threads >= 1);
        assert_eq!(diagnostics.effective_threads, 0);
        assert_eq!(diagnostics.resolved_mode, expected_mode);
        assert_eq!(diagnostics.fallback_reason, None);
    }

    #[test]
    fn reports_requested_and_effective_threads_after_task_clamp() {
        let diagnostics = resolve_runtime_diagnostics(
            3,
            RuntimeSchedulingOptions {
                thread_override: Some("8"),
                requested_mode: RequestedRuntimeMode::Auto,
            },
        );

        assert_eq!(diagnostics.requested_threads, 8);
        assert_eq!(diagnostics.effective_threads, 3);
        assert_eq!(diagnostics.resolved_mode, "threaded");
        assert_eq!(
            diagnostics.fallback_reason.as_deref(),
            Some("task-count-clamped")
        );
    }

    #[test]
    fn invalid_override_falls_back_to_native_resolution() {
        let fallback = resolve_runtime_diagnostics(
            2,
            RuntimeSchedulingOptions {
                thread_override: None,
                requested_mode: RequestedRuntimeMode::Auto,
            },
        );
        let diagnostics = resolve_runtime_diagnostics(
            2,
            RuntimeSchedulingOptions {
                thread_override: Some("invalid"),
                requested_mode: RequestedRuntimeMode::Auto,
            },
        );

        assert_eq!(diagnostics, fallback);
    }

    #[test]
    fn requested_mode_transitions_between_single_and_threaded() {
        let single = resolve_runtime_diagnostics(
            4,
            RuntimeSchedulingOptions {
                thread_override: Some("8"),
                requested_mode: RequestedRuntimeMode::Single,
            },
        );
        assert_eq!(single.requested_threads, 1);
        assert_eq!(single.effective_threads, 1);
        assert_eq!(single.resolved_mode, "single");
        assert_eq!(single.fallback_reason.as_deref(), Some("requested-single"));

        let threaded = resolve_runtime_diagnostics(
            4,
            RuntimeSchedulingOptions {
                thread_override: Some("8"),
                requested_mode: RequestedRuntimeMode::Threaded,
            },
        );
        assert_eq!(threaded.requested_threads, 8);
        assert_eq!(threaded.effective_threads, 4);
        assert_eq!(threaded.resolved_mode, "threaded");
        assert_eq!(
            threaded.fallback_reason.as_deref(),
            Some("task-count-clamped")
        );
    }

    #[test]
    fn waits_for_all_started_tasks_before_reporting_error() {
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
    fn uses_lowest_failing_index_regardless_of_completion_order() {
        let options = RuntimeSchedulingOptions {
            thread_override: Some("4"),
            requested_mode: RequestedRuntimeMode::Auto,
        };
        let seen: Arc<Vec<AtomicUsize>> =
            Arc::new((0..4).map(|_| AtomicUsize::new(0)).collect::<Vec<_>>());
        let allow_index_zero = Arc::new(AtomicBool::new(false));
        let task_seen = Arc::clone(&seen);
        let release_zero = Arc::clone(&allow_index_zero);
        let diagnostics = resolve_runtime_diagnostics(4, options);

        let error = assert_runtime_error(run_indexed_tasks(
            4,
            options,
            move |index| {
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
            },
        ));

        assert_eq!(error.index, 0);
        assert_eq!(error.error, ProbeError::CorruptData);
        assert_eq!(seen[0].load(Ordering::Relaxed), 1);
        assert_eq!(seen[3].load(Ordering::Relaxed), 1);
    }

    #[test]
    fn converts_static_otf_through_runtime_bridge() {
        let result = convert_otf_to_embedded_font(ConvertRequest {
            input_path: &fixture("testdata/cff-static.otf"),
            output_kind: OutputKind::Eot,
            variation_axes: None,
        })
        .expect("runtime bridge should encode static cff");

        assert!(
            !result.data.is_empty(),
            "runtime bridge should produce bytes"
        );
        assert_eq!(result.output_kind, OutputKind::Eot);
    }

    #[test]
    fn rejects_variation_axes_for_static_otf() {
        let error = convert_otf_to_embedded_font(ConvertRequest {
            input_path: &fixture("testdata/cff-static.otf"),
            output_kind: OutputKind::Fntdata,
            variation_axes: Some("wght=700"),
        })
        .expect_err("static CFF input should reject variation axes");

        assert_eq!(
            error,
            RuntimeError::Cff(CffError::VariationRejectedForStaticInput)
        );
    }

    #[test]
    fn rejects_variable_font_conversion_until_runtime_bridge_grows_full_support() {
        let error = convert_otf_to_embedded_font(ConvertRequest {
            input_path: &fixture("testdata/cff2-variable.otf"),
            output_kind: OutputKind::Fntdata,
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
}
