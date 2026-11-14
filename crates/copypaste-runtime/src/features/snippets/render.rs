//! Template rendering, lint and the bounded parse cache.
//!
//! A body is parsed once per revision and kept in a cache of at most
//! [`CACHE_ENTRIES`] templates, dropped on `Release` when a surface closes.
//! Rendering reads the clipboard and history only when a placeholder asks,
//! never re-parses a value, and needs no network.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Instant, SystemTime};

use copypaste_core::history_actions::{self, CopyOptions};
use copypaste_core::snippets::ranking::SearchTier;
use copypaste_core::storage::snippets::{SnippetDetail, MAX_BODY_BYTES};
use copypaste_ipc::snippets::{
    self as wire, ClipboardInput, LintSeverity, RenderSource, SnippetOp, TemplateFieldKind,
    TemplateLint,
};
use copypaste_ipc::{content_type, ContentClass, ErrorCode, HistoryQuery, Response, ResponseData};
use copypaste_template::{
    FieldKind, Issue, Lint, LintContext, Modifiers, NestedBody, NestedSnippet, Origin,
    RenderContext, RenderError, Required, Severity, SnippetRef, SnippetResolver, Template,
};

use crate::messages::{storage_error, MSG_NOT_FOUND};
use crate::Runtime;

/// Most parsed templates kept between renders.
const CACHE_ENTRIES: usize = 32;
/// Date placeholders fall back to this locale when the caller names none.
const DEFAULT_LOCALE: &str = "en";
/// History rows looked at for one `clipboard index=N`, secret ones included.
const MAX_HISTORY_SCAN: usize = 500;
const HISTORY_PAGE: u32 = 64;
/// Matches enough candidates that every exact duplicate is among them.
const RESOLVE_CANDIDATES: u32 = 8;

const MSG_OUTPUT_TOO_LARGE: &str = "The expanded snippet is larger than 1 MiB.";
const MSG_BODY_TOO_LARGE: &str = "The snippet is larger than the limit.";
const MSG_RENDER: &str = "The snippet could not be expanded.";

/// Parsed bodies keyed by body revision. A revision names one body text;
/// the source is compared on a hit anyway, so a stale entry is never used.
static CACHE: Mutex<Vec<(String, Arc<Template>)>> = Mutex::new(Vec::new());

fn cache() -> MutexGuard<'static, Vec<(String, Arc<Template>)>> {
    CACHE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The parsed body, from the cache when this revision was parsed before.
fn parsed(revision: Option<&str>, body: &str) -> Arc<Template> {
    let Some(revision) = revision else {
        return Arc::new(copypaste_template::parse(body));
    };
    {
        let mut entries = cache();
        if let Some(at) = entries
            .iter()
            .position(|(cached, template)| cached == revision && template.source() == body)
        {
            let entry = entries.remove(at);
            let template = Arc::clone(&entry.1);
            entries.push(entry);
            return template;
        }
    }
    let template = Arc::new(copypaste_template::parse(body));
    let mut entries = cache();
    if entries.len() >= CACHE_ENTRIES {
        entries.remove(0);
    }
    entries.push((revision.to_owned(), Arc::clone(&template)));
    template
}

/// Drops every cached template.
pub(super) fn release() {
    let mut entries = cache();
    entries.clear();
    entries.shrink_to_fit();
}

/// Where `clipboard index=0` reads from in one expansion.
pub(super) enum ClipboardText {
    None,
    Provided(String),
    /// The history item the user's clipboard holds, when known.
    Item(Option<String>),
    /// Only this text, for `clipboard index=0`; history is never read.
    Only(Option<String>),
}

impl ClipboardText {
    /// Resolves a wire input. `Host` looks past CopyPaste's own pending
    /// delivery, so a second use reads what the user copied, not the output.
    pub(super) fn of(runtime: &Runtime, input: ClipboardInput) -> Self {
        match input {
            ClipboardInput::None => Self::None,
            ClipboardInput::Provided { text } => Self::Provided(text),
            ClipboardInput::Host => Self::Item(runtime.user_clipboard_item()),
        }
    }
}

/// One expansion of a body.
pub(super) struct Expansion<'a> {
    pub(super) body: &'a str,
    pub(super) revision: Option<&'a str>,
    pub(super) template: bool,
    pub(super) origin: Option<Origin>,
    pub(super) concealed: bool,
    pub(super) values: &'a BTreeMap<String, String>,
    pub(super) clipboard: &'a ClipboardText,
    pub(super) locale: Option<&'a str>,
}

/// A snippet expanded without the clipboard, for a module.
pub(crate) struct IsolatedRendering {
    pub(crate) text: String,
    /// Required fields `values` left empty.
    pub(crate) missing_fields: Vec<String>,
    /// A clipboard placeholder was left empty.
    pub(crate) reads_clipboard: bool,
    /// A concealed snippet or a secret clip went into the text.
    pub(crate) secret: bool,
}

pub(super) struct Rendering {
    pub(super) text: String,
    pub(super) cursor_graphemes_from_end: Option<u32>,
    pub(super) warnings: Vec<Issue>,
    pub(super) missing_required: Vec<Required>,
    pub(super) unfilled: u32,
    /// A concealed snippet or a secret clip went into the text.
    pub(super) secret: bool,
}

impl Expansion<'_> {
    /// The expansion of a stored snippet.
    pub(super) fn of<'a>(
        detail: &'a SnippetDetail,
        values: &'a BTreeMap<String, String>,
        clipboard: &'a ClipboardText,
        locale: Option<&'a str>,
    ) -> Expansion<'a> {
        Expansion {
            body: &detail.body_text,
            revision: detail.revision.as_deref(),
            template: detail.snippet.template,
            origin: Some(Origin {
                id: detail.snippet.id.clone(),
                abbreviation: detail.snippet.abbreviation.clone(),
            }),
            concealed: detail.snippet.concealed,
            values,
            clipboard,
            locale,
        }
    }

    /// Expands once. With Template off the body is the output, as written.
    pub(super) fn render(&self, runtime: &Runtime) -> Result<Rendering, RenderError> {
        if !self.template {
            return Ok(Rendering {
                text: self.body.to_owned(),
                cursor_graphemes_from_end: None,
                warnings: Vec::new(),
                missing_required: Vec::new(),
                unfilled: 0,
                secret: self.concealed,
            });
        }
        let started = Instant::now();
        let template = parsed(self.revision, self.body);
        let library = Library::new(runtime);
        let secret = Cell::new(self.concealed);
        let clipboard = |index: u8| -> Option<String> {
            let (text, from_secret) = match (self.clipboard, index) {
                (ClipboardText::None, _) => return None,
                (ClipboardText::Provided(text), 0) => return Some(text.clone()),
                (ClipboardText::Only(text), 0) => return text.clone(),
                (ClipboardText::Only(_), _) => return None,
                (ClipboardText::Item(item), 0) => clip_text(runtime, item.as_deref()?)?,
                (ClipboardText::Provided(_), index) => history_text(runtime, index, None)?,
                (ClipboardText::Item(item), index) => {
                    history_text(runtime, index, item.as_deref())?
                }
            };
            if from_secret {
                secret.set(true);
            }
            Some(text)
        };
        let zone = local_zone();
        let rendered = copypaste_template::render(
            &template,
            &RenderContext {
                now: SystemTime::now(),
                local_zone: &zone,
                locale: self.locale.unwrap_or(DEFAULT_LOCALE),
                values: self.values,
                clipboard: &clipboard,
                snippets: &library,
                modifiers: &TransformModifiers,
                origin: self.origin.as_ref(),
            },
        )?;
        tracing::debug!(
            elapsed_ms = started.elapsed().as_millis() as u64,
            output_kib = rendered.text.len().div_ceil(1024),
            warnings = rendered.warnings.len(),
            "snippet rendered"
        );
        Ok(Rendering {
            cursor_graphemes_from_end: rendered
                .cursor_graphemes_from_end()
                .map(|steps| u32::try_from(steps).unwrap_or(u32::MAX)),
            secret: secret.get() || library.concealed.get(),
            text: rendered.text,
            warnings: rendered.warnings,
            missing_required: rendered.missing_required,
            unfilled: rendered.unfilled,
        })
    }
}

impl Runtime {
    /// Expands a stored snippet that reads no history: `clipboard` is the
    /// only text a clipboard placeholder can receive. Answers the text, the
    /// missing required inputs, and whether concealed content went in.
    pub(crate) fn render_snippet_isolated(
        &self,
        detail: &SnippetDetail,
        values: &BTreeMap<String, String>,
    ) -> Result<IsolatedRendering, RenderError> {
        let clipboard = ClipboardText::Only(None);
        let rendering = Expansion::of(detail, values, &clipboard, None).render(self)?;
        Ok(IsolatedRendering {
            reads_clipboard: rendering
                .warnings
                .iter()
                .any(|issue| matches!(issue, Issue::EmptyClipboard { .. })),
            missing_fields: rendering
                .missing_required
                .iter()
                .filter_map(|required| match required {
                    Required::Field { name } => Some(name.clone()),
                    Required::Clipboard { .. } => None,
                })
                .collect(),
            secret: rendering.secret,
            text: rendering.text,
        })
    }
}

pub(super) fn handle(runtime: &Runtime, id: u64, op: SnippetOp) -> Response {
    match op {
        SnippetOp::Render {
            source,
            values,
            clipboard,
            purpose: _,
            locale,
        } => {
            let clipboard = ClipboardText::of(runtime, clipboard);
            let rendered = match source {
                RenderSource::Snippet { id: snippet_id } => {
                    let detail = match runtime.store.snippet(&snippet_id) {
                        Ok(Some(detail)) => detail,
                        Ok(None) => return Response::err(id, ErrorCode::NotFound, MSG_NOT_FOUND),
                        Err(error) => return storage_error(id, "snippet_render", &error),
                    };
                    Expansion::of(&detail, &values, &clipboard, locale.as_deref()).render(runtime)
                }
                RenderSource::Body { text } => {
                    if text.len() > MAX_BODY_BYTES {
                        return Response::err(id, ErrorCode::ContentTooLarge, MSG_BODY_TOO_LARGE);
                    }
                    Expansion {
                        body: &text,
                        revision: None,
                        template: true,
                        origin: None,
                        concealed: false,
                        values: &values,
                        clipboard: &clipboard,
                        locale: locale.as_deref(),
                    }
                    .render(runtime)
                }
            };
            match rendered {
                Ok(rendering) => Response::ok(id, ResponseData::Rendered(wire_rendered(rendering))),
                Err(error) => render_error(id, &error),
            }
        }
        SnippetOp::Lint { body, template } => {
            if body.len() > MAX_BODY_BYTES {
                return Response::err(id, ErrorCode::ContentTooLarge, MSG_BODY_TOO_LARGE);
            }
            let library = Library::new(runtime);
            let lints = copypaste_template::lint(
                &copypaste_template::parse(&body),
                template,
                &LintContext {
                    locale: DEFAULT_LOCALE,
                    snippets: &library,
                    modifiers: &TransformModifiers,
                    origin: None,
                },
            );
            Response::ok(
                id,
                ResponseData::TemplateLints(lints.iter().map(wire_lint).collect()),
            )
        }
        _ => crate::features::unsupported(id),
    }
}

pub(super) fn render_error(id: u64, error: &RenderError) -> Response {
    match error {
        RenderError::OutputTooLarge => {
            Response::err(id, ErrorCode::ContentTooLarge, MSG_OUTPUT_TOO_LARGE)
        }
        RenderError::InstantOutOfRange => {
            tracing::warn!("a snippet was rendered with a clock out of range");
            Response::err(id, ErrorCode::Internal, MSG_RENDER)
        }
    }
}

/// A stored snippet with its fill-in form and lints.
pub(super) fn wire_detail(runtime: &Runtime, detail: SnippetDetail) -> wire::SnippetDetail {
    let template = parsed(detail.revision.as_deref(), &detail.body_text);
    let origin = Origin {
        id: detail.snippet.id.clone(),
        abbreviation: detail.snippet.abbreviation.clone(),
    };
    let library = Library::new(runtime);
    let fields = if detail.snippet.template {
        copypaste_template::fields(&template, Some(&origin), &library)
            .into_iter()
            .map(|field| wire::TemplateField {
                name: field.name,
                label: field.label,
                kind: match field.kind {
                    FieldKind::Text => TemplateFieldKind::Text,
                    FieldKind::Multiline => TemplateFieldKind::Multiline,
                    FieldKind::Choice => TemplateFieldKind::Choice,
                },
                default: field.default,
                options: field.options,
                required: field.required,
            })
            .collect()
    } else {
        Vec::new()
    };
    let lints = copypaste_template::lint(
        &template,
        detail.snippet.template,
        &LintContext {
            locale: DEFAULT_LOCALE,
            snippets: &library,
            modifiers: &TransformModifiers,
            origin: Some(&origin),
        },
    );
    wire::SnippetDetail {
        snippet: super::wire_summary(detail.snippet),
        body_text: detail.body_text,
        body_rich: detail.body_rich,
        revision: detail.revision.unwrap_or_default(),
        fields,
        lints: lints.iter().map(wire_lint).collect(),
    }
}

fn wire_rendered(rendering: Rendering) -> wire::Rendered {
    wire::Rendered {
        text: rendering.text,
        cursor_graphemes_from_end: rendering.cursor_graphemes_from_end,
        warnings: rendering
            .warnings
            .iter()
            .map(|issue| wire_issue(issue, 0, 0))
            .collect(),
        missing_required: rendering
            .missing_required
            .into_iter()
            .map(|required| match required {
                Required::Field { name } => name,
                Required::Clipboard { index } => format!("clipboard:{index}"),
            })
            .collect(),
        unfilled: rendering.unfilled,
    }
}

fn wire_lint(lint: &Lint) -> TemplateLint {
    let offset = |at: usize| u32::try_from(at).unwrap_or(u32::MAX);
    wire_issue(&lint.issue, offset(lint.span.start), offset(lint.span.end))
}

/// The issue's code and its one display argument (a name, key, modifier,
/// index or cycle), from the issue's own wire form.
fn wire_issue(issue: &Issue, start: u32, end: u32) -> TemplateLint {
    let value = serde_json::to_value(issue).unwrap_or_default();
    let code = value["code"].as_str().unwrap_or("malformed").to_owned();
    let detail = value
        .as_object()
        .and_then(|fields| fields.iter().find(|(key, _)| key.as_str() != "code"))
        .map(|(_, argument)| match argument {
            serde_json::Value::String(text) => text.clone(),
            serde_json::Value::Array(chain) => chain
                .iter()
                .filter_map(serde_json::Value::as_str)
                .collect::<Vec<_>>()
                .join(" → "),
            other => other.to_string(),
        });
    TemplateLint {
        severity: match issue.severity() {
            Severity::Error => LintSeverity::Error,
            Severity::Warning => LintSeverity::Warning,
            Severity::Hint => LintSeverity::Hint,
        },
        code,
        start,
        end,
        detail,
    }
}

/// The device zone as an IANA id, read where the OS keeps it; `UTC` when
/// it cannot be named.
fn local_zone() -> String {
    jiff::tz::TimeZone::try_system()
        .ok()
        .and_then(|zone| zone.iana_name().map(str::to_owned))
        .unwrap_or_else(|| "UTC".to_owned())
}

/// The plain text of a text clip, and whether it is secret.
fn clip_text(runtime: &Runtime, clip_id: &str) -> Option<(String, bool)> {
    let key = runtime.keyring.item_key();
    let opened = history_actions::open(&runtime.store, &key, clip_id).ok()?;
    if !content_type::is_text(&opened.row().content_type) {
        return None;
    }
    let origin = runtime.meta.origin_of(opened.row());
    let prepared = history_actions::prepare_copy(
        &runtime.store,
        &key,
        opened,
        CopyOptions {
            origin: &origin,
            local_device_id: runtime.meta.device_id(),
            plain_text: true,
            allow_content_uri: false,
            max_file_size_bytes: 0,
        },
    )
    .ok()?;
    let text = prepared.payload().plain_text()?.to_owned();
    let rule_secret = runtime
        .store
        .item_annotations(&[clip_id.to_owned()])
        .ok()
        .and_then(|mut found| found.remove(clip_id))
        .is_some_and(|annotations| annotations.rule_secret);
    Some((text, prepared.item.privacy.secret || rule_secret))
}

/// The `index`th previous text clip, newest first, past the one the
/// clipboard holds. Secret clips are skipped and never counted.
fn history_text(runtime: &Runtime, index: u8, skip: Option<&str>) -> Option<(String, bool)> {
    let query = HistoryQuery {
        content_classes: vec![ContentClass::Text],
        ..HistoryQuery::default()
    };
    let mut after = None;
    let mut counted = 0;
    let mut scanned = 0;
    loop {
        let page = runtime
            .store
            .query_history_bounded(
                &query,
                after.as_ref(),
                HISTORY_PAGE,
                copypaste_ipc::MAX_PAGE_CONTENT_BYTES,
            )
            .ok()?;
        let ids: Vec<String> = page.items.iter().map(|item| item.id.clone()).collect();
        let annotations = runtime.store.item_annotations(&ids).unwrap_or_default();
        for item in &page.items {
            scanned += 1;
            let masked = item.clipboard_privacy().secret
                || annotations.get(&item.id).is_some_and(|a| a.rule_secret);
            if masked || Some(item.id.as_str()) == skip {
                continue;
            }
            counted += 1;
            if counted == usize::from(index) {
                return clip_text(runtime, &item.id);
            }
        }
        if scanned >= MAX_HISTORY_SCAN || page.items.is_empty() {
            return None;
        }
        after = Some(page.next?);
    }
}

/// Live snippets for `snippet` placeholders, looked up once per reference
/// in one expansion.
struct Library<'a> {
    runtime: &'a Runtime,
    resolved: RefCell<Vec<(SnippetRef, Option<NestedSnippet>)>>,
    /// A concealed snippet was inserted.
    concealed: Cell<bool>,
}

impl<'a> Library<'a> {
    fn new(runtime: &'a Runtime) -> Self {
        Self {
            runtime,
            resolved: RefCell::default(),
            concealed: Cell::new(false),
        }
    }

    fn lookup(&self, reference: &SnippetRef) -> Option<(NestedSnippet, bool)> {
        let store = &self.runtime.store;
        let id = match reference {
            SnippetRef::Id(id) => id.clone(),
            SnippetRef::Abbreviation(abbreviation) => {
                store
                    .quick_search_snippets(abbreviation, RESOLVE_CANDIDATES)
                    .ok()?
                    .into_iter()
                    .find(|hit| hit.tier == SearchTier::ExactAbbreviation)?
                    .snippet
                    .id
            }
        };
        let detail = store
            .snippet(&id)
            .ok()
            .flatten()
            .filter(|detail| detail.snippet.deleted_at.is_none())?;
        let body = if detail.snippet.template {
            NestedBody::Template(parsed(detail.revision.as_deref(), &detail.body_text))
        } else {
            NestedBody::Literal(detail.body_text)
        };
        Some((
            NestedSnippet {
                origin: Origin {
                    id: detail.snippet.id,
                    abbreviation: detail.snippet.abbreviation,
                },
                body,
            },
            detail.snippet.concealed,
        ))
    }
}

impl SnippetResolver for Library<'_> {
    fn resolve(&self, reference: &SnippetRef) -> Option<NestedSnippet> {
        if let Some((_, found)) = self
            .resolved
            .borrow()
            .iter()
            .find(|(known, _)| known == reference)
        {
            return found.clone();
        }
        let found = self.lookup(reference).map(|(nested, concealed)| {
            if concealed {
                self.concealed.set(true);
            }
            nested
        });
        self.resolved
            .borrow_mut()
            .push((reference.clone(), found.clone()));
        found
    }
}

/// Template modifiers are the deterministic text transforms that name one.
struct TransformModifiers;

impl Modifiers for TransformModifiers {
    fn contains(&self, id: &str) -> bool {
        copypaste_transform::modifiers().any(|modifier| modifier.name == id)
    }

    fn apply(&self, id: &str, input: &str) -> Option<String> {
        copypaste_transform::apply_modifier(id, input).ok()
    }
}
