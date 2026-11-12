//! Built-in text transforms, pipelines and transform preferences.
//!
//! Every surface runs transforms here, so the daemon and Android produce the
//! same output. A run reads its source once and keeps only the current and
//! next text between steps; the first failing step stops the chain. Results
//! leave as a transient paste, a returned text, or a new clip that records
//! which transforms made it (ids and versions, never parameters). Logs carry
//! step counts, durations, sizes and error codes, never text.

mod defaults;
mod pipelines;
mod preview;

use std::borrow::Cow;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use copypaste_core::history_actions::{self, CopyOptions};
use copypaste_core::storage::organization::{
    ClipNoteRecord, Provenance, ProvenanceKind, Record, TransformRef,
};
use copypaste_core::{now_ms, ClipboardPayload, Ingested};
use copypaste_ipc::clips::Delivered;
use copypaste_ipc::content_type;
use copypaste_ipc::transforms::{
    TransformAction, TransformFlags, TransformInfo, TransformOp, TransformParam,
    TransformParamKind, TransformParamOption, TransformSource, TransformStep, TransformSteps,
};
use copypaste_ipc::{ClipboardPrivacy, ErrorCode, LibraryKinds, Response, ResponseData};
use copypaste_transform::url::tracking::TrackingRules;
use copypaste_transform::{Context, ParamIssue, ParamKind, Step, TransformError, Warning};

use super::delivery::{DeliveryError, DeliveryOptions};
use crate::messages::decrypt_error;
use crate::Runtime;

const MSG_TOO_LARGE: &str = "Too large to transform.";
const MSG_NOT_TEXT: &str = "Transforms work on text.";
const MSG_NO_SOURCE: &str = "Nothing to transform. Copy some text or pick a history item.";
const MSG_TOO_MANY_STEPS: &str = "A chain holds at most 16 steps.";
const MSG_RESULT_TOO_LARGE: &str = "The result is too large to deliver.";

/// Time a step that runs a user pattern may take, in a preview or a run.
const PATTERN_BUDGET: Duration = Duration::from_secs(2);
/// Time any other step of a run may take over the full input.
const RUN_STEP_BUDGET: Duration = Duration::from_secs(30);
/// Longest pipeline name a provenance note records, in chars.
const PROVENANCE_NAME_CHARS: usize = 64;

impl Runtime {
    pub(crate) fn transforms(&self, id: u64, op: TransformOp) -> Response {
        match op {
            TransformOp::Catalogue { locale } => Response::ok(
                id,
                ResponseData::TransformCatalogue(
                    copypaste_transform::describe(&locale)
                        .into_iter()
                        .map(catalogue_entry)
                        .collect(),
                ),
            ),
            TransformOp::Preview { .. } | TransformOp::Cancel { .. } => {
                preview::handle(self, id, op)
            }
            TransformOp::Run {
                source,
                steps_or_pipeline,
                action,
                bypass_default,
            } => self.run_transform(id, &source, steps_or_pipeline, action, bypass_default),
            TransformOp::Pipelines
            | TransformOp::SavePipeline { .. }
            | TransformOp::DeletePipeline { .. } => pipelines::handle(self, id, op),
            TransformOp::Suggest { .. }
            | TransformOp::Preferences
            | TransformOp::SetPreference { .. }
            | TransformOp::SetMember { .. }
            | TransformOp::DefaultFor { .. } => defaults::handle(self, id, op),
        }
    }

    /// Library records were written here since `since`: the transports send
    /// them and watchers get one coalesced `Library` event. History did not
    /// change, so no `Items` event is published.
    fn library_written(&self, since: i64, kinds: LibraryKinds) {
        self.p2p.node().note_local_version(since);
        self.p2p.wake();
        self.modules.note_version(since);
        self.on_org_changed(kinds);
    }

    /// The text `source` names. `want_html` asks for the HTML flavor too,
    /// for a chain whose first step reads rich input.
    fn transform_source(
        &self,
        id: u64,
        source: &TransformSource,
        want_html: bool,
    ) -> Result<Source, Refused> {
        let clip_id = match source {
            TransformSource::Text { text } => {
                if text.len() > copypaste_transform::MAX_INPUT_BYTES {
                    return Err(too_large(id).into());
                }
                return Ok(Source {
                    text: text.clone(),
                    ..Source::default()
                });
            }
            TransformSource::Clip { id: clip_id } => clip_id.clone(),
            TransformSource::Clipboard => match self.store.newest_live_key() {
                Ok(Some((clip_id, _))) => clip_id,
                Ok(None) => {
                    return Err(Response::err(id, ErrorCode::NotFound, MSG_NO_SOURCE).into())
                }
                Err(error) => {
                    return Err(crate::messages::storage_error(id, "transform", &error).into())
                }
            },
        };
        let key = self.keyring.item_key();
        let opened = history_actions::open(&self.store, &key, &clip_id)
            .map_err(|error| Box::new(error.response(id)))?;
        let row = opened.row();
        if !content_type::is_text(&row.content_type) {
            return Err(Response::err(id, ErrorCode::UnsupportedContent, MSG_NOT_TEXT).into());
        }
        let has_html = row.content_type == content_type::HTML;
        let html = if want_html && has_html {
            let payload =
                ClipboardPayload::open(row, &key).map_err(|e| Box::new(decrypt_error(id, &e)))?;
            payload.plain_text().map(str::to_owned)
        } else {
            None
        };
        let clip = SourceClip {
            id: clip_id.clone(),
            app_bundle_id: row.app_bundle_id.clone(),
            app_name: row.app_name.clone(),
        };
        let origin = self.meta.origin_of(row);
        let prepared = history_actions::prepare_copy(
            &self.store,
            &key,
            opened,
            CopyOptions {
                origin: &origin,
                local_device_id: self.meta.device_id(),
                plain_text: true,
                allow_content_uri: false,
                max_file_size_bytes: 0,
            },
        )
        .map_err(|error| Box::new(error.response(id)))?;
        let text = prepared
            .payload()
            .plain_text()
            .unwrap_or_default()
            .to_owned();
        let annotations = self
            .store
            .item_annotations(std::slice::from_ref(&clip_id))
            .unwrap_or_default()
            .remove(&clip_id)
            .unwrap_or_default();
        Ok(Source {
            text,
            html,
            has_html,
            sensitive: prepared.item.privacy.secret || annotations.rule_secret,
            hidden_from_ai: annotations.hidden_from_ai,
            clip: Some(clip),
        })
    }

    fn run_transform(
        &self,
        id: u64,
        source: &TransformSource,
        steps_or_pipeline: TransformSteps,
        action: TransformAction,
        bypass_default: bool,
    ) -> Response {
        let (steps, pipeline_name) = match steps_or_pipeline {
            TransformSteps::Steps { steps } => (steps, None),
            TransformSteps::Pipeline { id: pipeline_id } => {
                match pipelines::load(self, id, &pipeline_id) {
                    Ok(pipeline) => (pipeline.steps, Some(pipeline.name)),
                    Err(response) => return *response,
                }
            }
        };
        if steps.len() > copypaste_transform::MAX_STEPS {
            return Response::err(id, ErrorCode::InvalidRequest, MSG_TOO_MANY_STEPS);
        }
        let steps: Vec<_> = if bypass_default {
            Vec::new()
        } else {
            steps.iter().map(parse_step).collect()
        };
        let want_html = reads_rich_input(&steps);
        let source = match self.transform_source(id, source, want_html) {
            Ok(source) => source,
            Err(response) => return *response,
        };
        let tracking = defaults::tracking_rules(self, &steps);
        let env = Chain {
            cancel: None,
            tracking: tracking.as_ref(),
            preview: false,
        };
        let started = Instant::now();
        let mut sensitive = source.sensitive;
        let mut snapshot = Vec::new();
        let result = env.run(&steps, source.input(want_html), |index, run, _| {
            if let (
                StepRun::Ran {
                    sensitive: marked, ..
                },
                Ok(step),
            ) = (run, &steps[index])
            {
                sensitive |= marked;
                snapshot.push(TransformRef {
                    transform_id: step.transform_id.clone(),
                    version: step.version,
                });
            }
        });
        let output = match result {
            Ok(output) => output,
            Err((index, error)) => {
                tracing::debug!(
                    step = index + 1,
                    code = error.code(),
                    "transform run failed"
                );
                return step_failure(id, index, &error);
            }
        };
        tracing::debug!(
            steps = snapshot.len(),
            elapsed_ms = started.elapsed().as_millis() as u64,
            output_kib = output.len().div_ceil(1024),
            "transform run finished"
        );
        if output.len() > copypaste_ipc::MAX_CONTENT_BYTES {
            return Response::err(id, ErrorCode::ContentTooLarge, MSG_RESULT_TOO_LARGE);
        }
        // With no step run (a bypass, or every step disabled) a clip goes out
        // as itself, with every flavor it has.
        let untouched = snapshot
            .is_empty()
            .then(|| source.clip.as_ref().map(|clip| clip.id.clone()))
            .flatten();
        let provenance = (!snapshot.is_empty()).then(|| Provenance {
            kind: ProvenanceKind::Transformed,
            from_clip_id: source.clip.as_ref().map(|clip| clip.id.clone()),
            transform_snapshot: Some(snapshot),
            pipeline_name: pipeline_name.map(|name| bounded_name(&name)),
            grant_name: None,
        });
        match (action, untouched) {
            (TransformAction::Return, _) => Response::ok(
                id,
                ResponseData::Delivered(Delivered {
                    text: Some(output),
                    ..Delivered::default()
                }),
            ),
            (TransformAction::Paste, untouched) => {
                let delivered = match &untouched {
                    Some(clip_id) => self
                        .deliver_item(clip_id, false, false)
                        .map_err(DeliveryError::from),
                    None => self.deliver_text(
                        &output,
                        DeliveryOptions {
                            secret: sensitive,
                            transient: true,
                            record_in_history: false,
                            html: false,
                        },
                    ),
                };
                match delivered {
                    Ok(delivered) => self.pasted(id, delivered.token),
                    Err(error) => error.response(id),
                }
            }
            (TransformAction::Copy, Some(clip_id)) => {
                match self.deliver_item(&clip_id, false, true) {
                    Ok(_) => self.get(id, &clip_id),
                    Err(error) => error.response(id),
                }
            }
            (TransformAction::Copy | TransformAction::SaveNewClip, _) => {
                let saved = match self.save_result(id, &source, &output, sensitive, provenance) {
                    Ok(saved) => saved,
                    Err(response) => return *response,
                };
                if action == TransformAction::Copy {
                    if let Err(error) = self.deliver_item(&saved.id, false, true) {
                        return error.response(id);
                    }
                } else {
                    self.note_local_change_since(saved.created_at);
                }
                self.get(id, &saved.id)
            }
        }
    }

    /// A transient paste. The restore token comes back only while the user
    /// keeps "restore the clipboard after a transformed paste" on; the caller
    /// restores with it once its native paste is done.
    fn pasted(&self, id: u64, token: Option<String>) -> Response {
        let restore = defaults::restore_after_paste(self);
        Response::ok(
            id,
            ResponseData::Delivered(Delivered {
                token: token.filter(|_| restore),
                ..Delivered::default()
            }),
        )
    }

    /// Stores `text` as a new plain-text clip that keeps the source's
    /// application and sensitivity, with provenance when a transform ran.
    /// Identical text already in history is that clip, unchanged.
    fn save_result(
        &self,
        id: u64,
        source: &Source,
        text: &str,
        sensitive: bool,
        provenance: Option<Provenance>,
    ) -> Result<copypaste_core::StoredItem, Refused> {
        let metadata = sensitive.then(|| copypaste_core::PayloadMetadata {
            privacy: ClipboardPrivacy {
                secret: true,
                transient: false,
            },
            ..Default::default()
        });
        let clip = source.clip.as_ref();
        let settings = self.settings.config();
        let ingested =
            copypaste_core::ingest_into_with_capture_source_metadata_with_current_retention(
                &self.store,
                &self.keyring,
                text,
                content_type::TEXT,
                now_ms(),
                clip.and_then(|clip| clip.app_bundle_id.as_deref()),
                clip.and_then(|clip| clip.app_name.as_deref()),
                metadata.as_ref(),
                &settings,
                || self.settings.config(),
            )
            .map_err(|error| Box::new(DeliveryError::Ingest(error).response(id)))?;
        let Ingested::Stored(item) = ingested else {
            return Ok(ingested.into_item());
        };
        if provenance.is_some() || source.hidden_from_ai {
            let note = ClipNoteRecord {
                clip_id: item.id.clone(),
                hidden_from_ai: source.hidden_from_ai,
                hidden_changed_at: if source.hidden_from_ai { now_ms() } else { 0 },
                base_stamp: 0,
                keep: false,
                expires_at: None,
                expiry_duration_ms: None,
                provenance,
            };
            if let Err(error) = self
                .store
                .write_organization_record(&self.keyring.item_key(), Record::ClipNote(note))
            {
                tracing::warn!(?error, "a transformed clip was saved without its note");
            }
        }
        Ok(item)
    }
}

/// A refusal to answer with; boxed, since a response is large.
type Refused = Box<Response>;

/// What a transform reads, and what results derived from it inherit.
#[derive(Default)]
struct Source {
    text: String,
    /// The HTML flavor, loaded only for a chain that starts with rich input.
    html: Option<String>,
    has_html: bool,
    sensitive: bool,
    hidden_from_ai: bool,
    clip: Option<SourceClip>,
}

impl Source {
    fn input(&self, want_html: bool) -> &str {
        match &self.html {
            Some(html) if want_html => html,
            _ => &self.text,
        }
    }
}

struct SourceClip {
    id: String,
    app_bundle_id: Option<String>,
    app_name: Option<String>,
}

/// What one step of a chain did.
enum StepRun {
    /// Disabled: its input passed through unchanged.
    Disabled,
    Ran {
        warnings: Vec<Warning>,
        sensitive: bool,
    },
    Failed(TransformError),
}

/// How a chain runs: its cancellation flag, the user's tracking lists, and
/// whether it is a bounded preview.
struct Chain<'a> {
    cancel: Option<&'a AtomicBool>,
    tracking: Option<&'a TrackingRules>,
    preview: bool,
}

impl Chain<'_> {
    fn context(&self, step: &Step) -> Context<'_> {
        let pattern = copypaste_transform::catalogue()
            .get(&step.transform_id)
            .is_some_and(|def| def.flags().needs_free_text);
        let budget = if pattern || self.preview {
            PATTERN_BUDGET
        } else {
            RUN_STEP_BUDGET
        };
        let mut context = Context::new().with_deadline(Instant::now() + budget);
        if let Some(cancel) = self.cancel {
            context = context.with_cancel(cancel);
        }
        if let Some(tracking) = self.tracking {
            context = context.with_tracking(tracking);
        }
        context
    }

    /// Runs `steps` over `input` in order and reports each step with the
    /// text it left. The first failing step stops the chain.
    fn run(
        &self,
        steps: &[Result<Step, TransformError>],
        input: &str,
        mut visit: impl FnMut(usize, StepRun, &str),
    ) -> Result<String, (usize, TransformError)> {
        let mut current = Cow::Borrowed(input);
        for (index, step) in steps.iter().enumerate() {
            let result = match step {
                Ok(step) if !step.enabled => {
                    visit(index, StepRun::Disabled, &current);
                    continue;
                }
                Ok(step) => copypaste_transform::apply(step, &current, &self.context(step)),
                Err(error) => Err(error.clone()),
            };
            match result {
                Ok(output) => {
                    current = Cow::Owned(output.text);
                    let run = StepRun::Ran {
                        warnings: output.warnings,
                        sensitive: output.sensitive,
                    };
                    visit(index, run, &current);
                }
                Err(error) => {
                    visit(index, StepRun::Failed(error.clone()), "");
                    return Err((index, error));
                }
            }
        }
        Ok(current.into_owned())
    }
}

/// A wire step as the transform crate runs it. Parameters that are not a
/// JSON object fail the step, not the request, so a preview can show where.
fn parse_step(step: &TransformStep) -> Result<Step, TransformError> {
    let params = if step.params_json.trim().is_empty() {
        serde_json::Map::new()
    } else {
        serde_json::from_str(&step.params_json).map_err(|_| TransformError::InvalidParam {
            name: "params".to_owned(),
            issue: ParamIssue::WrongType,
        })?
    };
    Ok(Step {
        transform_id: step.transform_id.clone(),
        version: step.version,
        params,
        enabled: step.enabled,
    })
}

/// Whether the first enabled step reads the HTML flavor of a rich item.
fn reads_rich_input(steps: &[Result<Step, TransformError>]) -> bool {
    steps
        .iter()
        .find_map(|step| step.as_ref().ok().filter(|step| step.enabled))
        .and_then(|step| copypaste_transform::catalogue().get(&step.transform_id))
        .is_some_and(|def| def.flags().rich_input)
}

fn too_large(id: u64) -> Response {
    Response::err(id, ErrorCode::ContentTooLarge, MSG_TOO_LARGE)
}

/// "Step N failed: code", with the input position when there is one.
fn step_failure(id: u64, index: usize, error: &TransformError) -> Response {
    let code = match error {
        TransformError::UnknownTransform { .. } | TransformError::UnsupportedVersion { .. } => {
            ErrorCode::Unsupported
        }
        TransformError::InputTooLarge { .. } | TransformError::OutputTooLarge { .. } => {
            ErrorCode::ContentTooLarge
        }
        _ => ErrorCode::InvalidRequest,
    };
    let mut message = format!("Step {} failed: {}", index + 1, error.code());
    if let TransformError::InvalidInput {
        location: Some(location),
        ..
    } = error
    {
        message.push_str(&format!(
            " at line {}, column {}",
            location.line, location.column
        ));
    }
    Response::err(id, code, message)
}

fn bounded_name(name: &str) -> String {
    name.chars()
        .filter(|c| !c.is_control())
        .take(PROVENANCE_NAME_CHARS)
        .collect()
}

fn catalogue_entry(info: copypaste_transform::TransformInfo) -> TransformInfo {
    TransformInfo {
        id: info.id.to_owned(),
        version: info.version,
        category: info.category.key().to_owned(),
        name: info.name,
        description: info.description,
        example_in: info.example_in.to_owned(),
        example_out: info.example_out.to_owned(),
        keywords: info.keywords,
        params: info.params.into_iter().map(catalogue_param).collect(),
        flags: TransformFlags {
            can_be_default: info.flags.can_be_default,
            needs_free_text: info.flags.needs_free_text,
            inherits_sensitivity: info.flags.always_sensitive,
            rich_input: info.flags.rich_input,
            deterministic: info.flags.deterministic,
            modifier: info.modifier.is_some(),
        },
    }
}

fn catalogue_param(param: copypaste_transform::ParamInfo) -> TransformParam {
    use serde_json::Value;
    let (kind, default, min, max, max_len) = match param.kind {
        ParamKind::Bool { default } => (
            TransformParamKind::Bool,
            Value::from(default),
            None,
            None,
            None,
        ),
        ParamKind::Choice { default, .. } => (
            TransformParamKind::Choice,
            Value::from(default),
            None,
            None,
            None,
        ),
        ParamKind::Int { min, max, default } => (
            TransformParamKind::Integer,
            Value::from(default),
            Some(min),
            Some(max),
            None,
        ),
        ParamKind::Text { max_bytes, default } => (
            TransformParamKind::Text,
            Value::from(default),
            None,
            None,
            u32::try_from(max_bytes).ok(),
        ),
    };
    TransformParam {
        key: param.name.to_owned(),
        label: param.label,
        kind,
        default_json: default.to_string(),
        options: param
            .options
            .into_iter()
            .map(|option| TransformParamOption {
                value: option.value.to_owned(),
                label: option.label,
            })
            .collect(),
        min,
        max,
        max_len,
    }
}
