//! Bounded, cancellable transform previews.
//!
//! A preview runs the chain over the first 64 KiB of its source and returns
//! at most 64 KiB of each step's output; only a run transforms the whole
//! input. Each preview registers a cancel flag under its request id while it
//! runs. `Cancel` sets it, and a newer preview on the same runtime sets the
//! older one's, so a superseded chain stops between steps and inside long
//! ones. Nothing is kept once the preview returns.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use copypaste_ipc::transforms::{
    StepResult, TextCounts, TransformFailure, TransformOp, TransformPreview, TransformSource,
    TransformStep, TransformWarning,
};
use copypaste_ipc::{ErrorCode, Response, ResponseData};
use copypaste_transform::{preview_prefix, TransformError, Warning, PREVIEW_BYTES};

use super::{defaults, parse_step, reads_rich_input, Chain, StepRun, MSG_TOO_MANY_STEPS};
use crate::Runtime;

/// Previews running in this process, keyed by runtime and request id.
static IN_FLIGHT: Mutex<Vec<InFlight>> = Mutex::new(Vec::new());

struct InFlight {
    runtime: usize,
    request_id: String,
    cancel: Arc<AtomicBool>,
}

fn in_flight() -> MutexGuard<'static, Vec<InFlight>> {
    IN_FLIGHT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn runtime_key(runtime: &Runtime) -> usize {
    std::ptr::from_ref(runtime).addr()
}

/// A running preview's cancel flag; dropping it unregisters the preview.
struct Registration {
    cancel: Arc<AtomicBool>,
}

impl Drop for Registration {
    fn drop(&mut self) {
        in_flight().retain(|flight| !Arc::ptr_eq(&flight.cancel, &self.cancel));
    }
}

/// Registers a preview and cancels the older ones of the same runtime.
fn register(runtime: &Runtime, request_id: &str) -> Registration {
    let key = runtime_key(runtime);
    let cancel = Arc::new(AtomicBool::new(false));
    let mut flights = in_flight();
    for older in flights.iter().filter(|flight| flight.runtime == key) {
        older.cancel.store(true, Ordering::Relaxed);
    }
    flights.push(InFlight {
        runtime: key,
        request_id: request_id.to_owned(),
        cancel: Arc::clone(&cancel),
    });
    Registration { cancel }
}

/// Whether a running preview with this id was told to stop.
fn cancel(runtime: &Runtime, request_id: &str) -> bool {
    let key = runtime_key(runtime);
    let mut found = false;
    for flight in in_flight()
        .iter()
        .filter(|flight| flight.runtime == key && flight.request_id == request_id)
    {
        flight.cancel.store(true, Ordering::Relaxed);
        found = true;
    }
    found
}

pub(super) fn handle(runtime: &Runtime, id: u64, op: TransformOp) -> Response {
    match op {
        TransformOp::Preview {
            request_id,
            source,
            steps,
        } => preview(runtime, id, request_id, &source, &steps),
        TransformOp::Cancel { request_id } => {
            cancel(runtime, &request_id);
            Response::ok(id, ResponseData::Empty {})
        }
        _ => crate::features::unsupported(id),
    }
}

fn preview(
    runtime: &Runtime,
    id: u64,
    request_id: String,
    source: &TransformSource,
    steps: &[TransformStep],
) -> Response {
    if steps.len() > copypaste_transform::MAX_STEPS {
        return Response::err(id, ErrorCode::InvalidRequest, MSG_TOO_MANY_STEPS);
    }
    let registration = register(runtime, &request_id);
    let parsed: Vec<_> = steps.iter().map(parse_step).collect();
    let want_html = reads_rich_input(&parsed);
    let source = match runtime.transform_source(id, source, want_html) {
        Ok(source) => source,
        Err(response) => return *response,
    };
    let full = source.input(want_html);
    let input = preview_prefix(full);
    let tracking = defaults::tracking_rules(runtime, &parsed);
    let chain = Chain {
        cancel: Some(&registration.cancel),
        tracking: tracking.as_ref(),
        preview: true,
    };
    let mut truncated = input.len() < full.len();
    let mut sensitive = source.sensitive;
    let mut results = Vec::with_capacity(steps.len());
    let outcome = chain.run(&parsed, input, |_, run, text| {
        results.push(match run {
            StepRun::Disabled => StepResult {
                not_run: true,
                ..shown(text)
            },
            StepRun::Ran {
                warnings,
                sensitive: marked,
            } => {
                sensitive |= marked;
                truncated |= text.len() > PREVIEW_BYTES;
                StepResult {
                    warnings: warnings.iter().map(warning).collect(),
                    ..shown(text)
                }
            }
            StepRun::Failed(error) => StepResult {
                error: Some(failure(&error)),
                ..shown("")
            },
        });
    });
    if let Err((index, error)) = &outcome {
        tracing::debug!(
            step = index + 1,
            code = error.code(),
            "transform preview stopped"
        );
    }
    results.resize_with(steps.len(), || StepResult {
        not_run: true,
        ..shown("")
    });
    Response::ok(
        id,
        ResponseData::TransformPreview(TransformPreview {
            request_id,
            input_counts: TextCounts {
                bytes: full.len() as u64,
                ..counts(input)
            },
            steps: results,
            truncated,
            no_changes: outcome.is_ok_and(|output| output == input),
            sensitive,
        }),
    )
}

/// Counts of the shown prefix; `bytes` is the size of the whole text.
fn counts(text: &str) -> TextCounts {
    let shown = copypaste_transform::counts(preview_prefix(text));
    TextCounts {
        chars: shown.characters as u64,
        words: shown.words as u64,
        lines: shown.lines as u64,
        bytes: text.len() as u64,
    }
}

fn shown(text: &str) -> StepResult {
    StepResult {
        output_prefix: preview_prefix(text).to_owned(),
        counts: counts(text),
        warnings: Vec::new(),
        error: None,
        not_run: false,
    }
}

fn warning(warning: &Warning) -> TransformWarning {
    TransformWarning {
        code: warning.code.to_owned(),
        detail: warning
            .location
            .map(|location| format!("{}:{}", location.line, location.column)),
    }
}

fn failure(error: &TransformError) -> TransformFailure {
    let location = match error {
        TransformError::InvalidInput { location, .. } => *location,
        _ => None,
    };
    TransformFailure {
        code: error.code().to_owned(),
        offset: location.map(|location| location.offset as u64),
        line: location.and_then(|location| u32::try_from(location.line).ok()),
        column: location.and_then(|location| u32::try_from(location.column).ok()),
    }
}

#[cfg(test)]
mod tests {
    use copypaste_transform::Step;

    use super::*;
    use crate::tests::fixture;

    #[test]
    fn a_cancelled_preview_stops_and_a_newer_one_supersedes_the_older() {
        let (runtime, _dir) = fixture();
        let older = register(&runtime, "older");
        let newer = register(&runtime, "newer");
        assert!(older.cancel.load(Ordering::Relaxed));
        assert!(!newer.cancel.load(Ordering::Relaxed));
        assert!(cancel(&runtime, "newer"));

        let chain = Chain {
            cancel: Some(&newer.cancel),
            tracking: None,
            preview: true,
        };
        let steps = [
            Ok(Step::current("case.upper").unwrap()),
            Ok(Step::current("case.lower").unwrap()),
        ];
        let mut seen = Vec::new();
        let outcome = chain.run(&steps, "abc", |index, run, _| {
            seen.push((
                index,
                matches!(run, StepRun::Failed(TransformError::Cancelled)),
            ));
        });
        assert_eq!(outcome, Err((0, TransformError::Cancelled)));
        assert_eq!(seen, [(0, true)]);

        drop((older, newer));
        assert!(
            !cancel(&runtime, "newer"),
            "a finished preview is forgotten"
        );
    }
}
