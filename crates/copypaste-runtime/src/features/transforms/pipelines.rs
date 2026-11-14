//! Saved transform pipelines.
//!
//! A pipeline's steps are one atomic synced field, so concurrent edits keep
//! one device's whole list. A step this build cannot run (an unknown id or
//! version from a newer build) is listed and stored verbatim, and a run that
//! reaches it fails instead of skipping it.

use copypaste_core::now_ms;
use copypaste_core::storage::organization::PipelineRecord;
use copypaste_core::storage::transforms::PipelineWrite;
use copypaste_core::StoreError;
use copypaste_ipc::transforms::{Pipeline, PipelineDraft, TransformOp, TransformStep};
use copypaste_ipc::{ErrorCode, LibraryKinds, Response, ResponseData};
use copypaste_transform::{Step, TransformError};
use serde_json::{Map, Value};

use super::{parse_step, Refused};
use crate::messages::storage_error;
use crate::Runtime;

const MAX_NAME_CHARS: usize = 80;
const MAX_DESCRIPTION_CHARS: usize = 512;
const MSG_NOT_FOUND: &str = "That pipeline no longer exists.";
const MSG_NAME: &str = "Name the pipeline with up to 80 characters on one line.";
const MSG_NAME_TAKEN: &str = "Another pipeline already has this name.";
const MSG_DESCRIPTION: &str = "Keep the description to 512 characters on one line.";
const MSG_STEPS: &str = "A pipeline holds 1 to 16 steps.";
const MSG_TOO_LARGE: &str = "The pipeline is too large to save.";

pub(super) fn handle(runtime: &Runtime, id: u64, op: TransformOp) -> Response {
    match op {
        TransformOp::Pipelines => match runtime.store.transform_pipelines() {
            Ok(records) => Response::ok(
                id,
                ResponseData::Pipelines(records.into_iter().map(wire).collect()),
            ),
            Err(error) => storage_error(id, "transform_pipelines", &error),
        },
        TransformOp::SavePipeline { draft } => save(runtime, id, &draft),
        TransformOp::DeletePipeline { id: pipeline_id } => {
            let started = now_ms();
            match runtime.store.delete_transform_pipeline(
                &runtime.keyring.item_key(),
                &pipeline_id,
                started,
            ) {
                Ok(true) => {
                    runtime.library_written(started, LibraryKinds::PIPELINES);
                    Response::ok(id, ResponseData::Empty {})
                }
                Ok(false) => Response::err(id, ErrorCode::NotFound, MSG_NOT_FOUND),
                Err(error) => storage_error(id, "delete_pipeline", &error),
            }
        }
        _ => crate::features::unsupported(id),
    }
}

/// A live pipeline, for a run.
pub(super) fn load(runtime: &Runtime, id: u64, pipeline_id: &str) -> Result<Pipeline, Refused> {
    match runtime.store.live_transform_pipeline(pipeline_id) {
        Ok(Some(record)) => Ok(wire(record)),
        Ok(None) => Err(Response::err(id, ErrorCode::NotFound, MSG_NOT_FOUND).into()),
        Err(error) => Err(storage_error(id, "transform_pipeline", &error).into()),
    }
}

/// Whether every step of a live pipeline can run here as a paste default:
/// known, current, valid, deterministic and allowed as a default.
pub(super) fn usable_as_default(runtime: &Runtime, pipeline_id: &str) -> bool {
    let Ok(Some(record)) = runtime.store.live_transform_pipeline(pipeline_id) else {
        return false;
    };
    let steps = wire(record).steps;
    !steps.is_empty()
        && steps.iter().all(|step| {
            parse_step(step).is_ok_and(|parsed| {
                copypaste_transform::validate_step(&parsed).is_ok()
                    && copypaste_transform::catalogue()
                        .get(&parsed.transform_id)
                        .is_some_and(|def| def.flags().deterministic && def.flags().can_be_default)
            })
        })
}

fn wire(record: PipelineRecord) -> Pipeline {
    Pipeline {
        steps: wire_steps(&record.steps_json),
        id: record.id,
        name: record.name,
        description: record.description,
        deleted_at: record.deleted_at,
    }
}

/// Reads stored steps leniently: a field this build does not expect keeps
/// the step listed, as one that cannot run, rather than hiding it.
fn wire_steps(steps_json: &str) -> Vec<TransformStep> {
    let steps: Vec<Map<String, Value>> = serde_json::from_str(steps_json).unwrap_or_default();
    steps
        .into_iter()
        .map(|step| TransformStep {
            transform_id: step
                .get("transform_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            version: step
                .get("version")
                .and_then(Value::as_u64)
                .and_then(|version| u32::try_from(version).ok())
                .unwrap_or_default(),
            params_json: step
                .get("params")
                .filter(|params| params.is_object())
                .map_or_else(|| "{}".to_owned(), Value::to_string),
            enabled: step.get("enabled").and_then(Value::as_bool).unwrap_or(true),
        })
        .collect()
}

fn single_line(text: &str, max_chars: usize) -> bool {
    text.chars().count() <= max_chars && !text.chars().any(char::is_control)
}

/// Steps as stored. Parameters of a step this build runs are checked; any
/// other step is kept as written.
fn stored_steps(steps: &[TransformStep]) -> Result<String, &'static str> {
    if steps.is_empty() || steps.len() > copypaste_transform::MAX_STEPS {
        return Err(MSG_STEPS);
    }
    let mut stored: Vec<Step> = Vec::with_capacity(steps.len());
    for step in steps {
        let parsed = parse_step(step).map_err(|_| MSG_STEPS)?;
        if parsed.transform_id.is_empty() || !single_line(&parsed.transform_id, 128) {
            return Err(MSG_STEPS);
        }
        let runnable = copypaste_transform::catalogue()
            .get(&parsed.transform_id)
            .is_some_and(|def| def.version() == parsed.version);
        if runnable {
            if let Err(error @ TransformError::InvalidParam { .. }) =
                copypaste_transform::validate_step(&parsed)
            {
                tracing::debug!(code = error.code(), "pipeline step refused");
                return Err(MSG_STEPS);
            }
        }
        stored.push(parsed);
    }
    serde_json::to_string(&stored).map_err(|_| MSG_STEPS)
}

fn save(runtime: &Runtime, id: u64, draft: &PipelineDraft) -> Response {
    let invalid = |message: &str| Response::err(id, ErrorCode::InvalidRequest, message);
    let name = draft.name.trim();
    if name.is_empty() || !single_line(name, MAX_NAME_CHARS) {
        return invalid(MSG_NAME);
    }
    let description = draft
        .description
        .as_deref()
        .map(str::trim)
        .filter(|description| !description.is_empty());
    if description.is_some_and(|text| !single_line(text, MAX_DESCRIPTION_CHARS)) {
        return invalid(MSG_DESCRIPTION);
    }
    let steps_json = match stored_steps(&draft.steps) {
        Ok(json) => json,
        Err(message) => return invalid(message),
    };
    let taken = match runtime.store.transform_pipelines() {
        Ok(existing) => existing.iter().any(|other| {
            Some(other.id.as_str()) != draft.id.as_deref()
                && other.name.to_lowercase() == name.to_lowercase()
        }),
        Err(error) => return storage_error(id, "transform_pipelines", &error),
    };
    if taken {
        return invalid(MSG_NAME_TAKEN);
    }
    let started = now_ms();
    let saved = runtime.store.save_transform_pipeline(
        &runtime.keyring.item_key(),
        PipelineWrite {
            id: draft.id.as_deref(),
            name,
            description,
            steps_json: &steps_json,
        },
    );
    match saved {
        Ok(record) => {
            runtime.library_written(started, LibraryKinds::PIPELINES);
            Response::ok(id, ResponseData::Pipeline(wire(record)))
        }
        Err(StoreError::InvalidHistoryQuery) => invalid(MSG_TOO_LARGE),
        Err(error) => storage_error(id, "save_pipeline", &error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(transform_id: &str, version: u32, params_json: &str) -> TransformStep {
        TransformStep {
            transform_id: transform_id.into(),
            version,
            params_json: params_json.into(),
            enabled: true,
        }
    }

    #[test]
    fn stored_steps_keep_unknown_steps_and_refuse_bad_known_parameters() {
        let unknown = step("future.step", 7, r#"{"depth":2}"#);
        let json = stored_steps(&[unknown.clone(), step("case.upper", 1, "")]).unwrap();
        assert_eq!(wire_steps(&json)[0], unknown);
        assert_eq!(wire_steps(&json)[1].params_json, "{}");
        assert!(stored_steps(&[step("case.upper", 1, r#"{"nope":1}"#)]).is_err());
        assert!(stored_steps(&[step("case.upper", 1, "[1]")]).is_err());
        assert!(stored_steps(&[]).is_err());
        assert_eq!(
            wire_steps(r#"[{"op":"from-a-newer-build"}]"#)[0].transform_id,
            ""
        );
    }
}
