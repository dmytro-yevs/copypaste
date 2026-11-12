//! The device-local paste queue and its settings.

use copypaste_ipc::config::{PasteQueueSettings, QueueEndBehavior, QueueSeparator};
use copypaste_ipc::paste_queue::{self as ipc, PasteQueueOp};
use copypaste_ipc::{ConfigPatch, Method, ResponseData};

use super::snippets::{template_values, TemplateValue};
use super::{expect_data, RuntimeError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteQueuePhase {
    Empty,
    Active,
    Finished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteQueueEntryStatus {
    Pending,
    Pasted,
    Skipped,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteQueueContentKind {
    Text,
    RichText,
    Image,
    Files,
    Link,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteQueueAddedVia {
    Collect,
    HistoryAdd,
    SnippetAdd,
    SelectionAction,
    Share,
}

#[derive(Debug, Clone)]
pub struct PasteQueueEntryView {
    pub entry_id: String,
    pub position_label: String,
    pub status: PasteQueueEntryStatus,
    pub content_kind: PasteQueueContentKind,
    pub preview: String,
    pub masked: bool,
    pub edited: bool,
    pub source_app_bundle_id: Option<String>,
    pub source_app_name: Option<String>,
    pub source_app_icon_id: Option<String>,
    pub added_via: PasteQueueAddedVia,
    pub rendered_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteQueueEndBehavior {
    Stop,
    Loop,
    KeepLast,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteQueueSeparator {
    NewLine,
    BlankLine,
    Tab,
    Space,
    Comma,
    CommaSpace,
    None,
    Custom,
}

#[derive(Debug, Clone)]
pub struct PasteQueuePreferences {
    pub enabled: bool,
    pub insert_at_start: bool,
    pub end_behavior: PasteQueueEndBehavior,
    pub separator: PasteQueueSeparator,
    pub custom_separator: String,
    pub plain_text: bool,
    pub idle_stop_minutes: u32,
    pub max_entries: u32,
    pub move_to_next_field: bool,
    pub restore_when_finished: bool,
}

#[derive(Debug, Clone)]
pub struct PasteQueueView {
    pub revision: u64,
    pub collecting: bool,
    pub phase: PasteQueuePhase,
    pub full: bool,
    pub entries: Vec<PasteQueueEntryView>,
    pub stale_since_ms: Option<i64>,
    pub undo_until_ms: Option<i64>,
    pub settings: PasteQueuePreferences,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteQueueSourceKind {
    Clip,
    Snippet,
    SelectionText,
    SharedText,
}

/// One thing to add: `id` for a clip or snippet, `text` for shared text,
/// `values` for a snippet's template fields.
#[derive(Debug, Clone)]
pub struct PasteQueueSourceInput {
    pub kind: PasteQueueSourceKind,
    pub id: Option<String>,
    pub text: Option<String>,
    pub values: Vec<TemplateValue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteQueueTargetKind {
    Next,
    All,
    Entry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteQueueDeliveryMode {
    Clipboard,
    Return,
    CopyOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteQueueNothingToPaste {
    Empty,
    Finished,
    Mixed,
}

#[derive(Debug, Clone)]
pub struct PasteQueueDeliveryView {
    pub token: Option<String>,
    pub entry_ids: Vec<String>,
    pub text: Option<String>,
    pub unavailable: u32,
    pub finished: bool,
    pub nothing_to_paste: Option<PasteQueueNothingToPaste>,
}

fn preferences(value: PasteQueueSettings) -> PasteQueuePreferences {
    PasteQueuePreferences {
        enabled: value.enabled,
        insert_at_start: value.insert_at_start,
        end_behavior: match value.end_behavior {
            QueueEndBehavior::Stop => PasteQueueEndBehavior::Stop,
            QueueEndBehavior::Loop => PasteQueueEndBehavior::Loop,
            QueueEndBehavior::KeepLast => PasteQueueEndBehavior::KeepLast,
        },
        separator: match value.separator {
            QueueSeparator::NewLine => PasteQueueSeparator::NewLine,
            QueueSeparator::BlankLine => PasteQueueSeparator::BlankLine,
            QueueSeparator::Tab => PasteQueueSeparator::Tab,
            QueueSeparator::Space => PasteQueueSeparator::Space,
            QueueSeparator::Comma => PasteQueueSeparator::Comma,
            QueueSeparator::CommaSpace => PasteQueueSeparator::CommaSpace,
            QueueSeparator::None => PasteQueueSeparator::None,
            QueueSeparator::Custom => PasteQueueSeparator::Custom,
        },
        custom_separator: value.custom_separator,
        plain_text: value.plain_text,
        idle_stop_minutes: value.idle_stop_minutes,
        max_entries: value.max_entries,
        move_to_next_field: value.move_to_next_field,
        restore_when_finished: value.restore_when_finished,
    }
}

fn settings(value: PasteQueuePreferences) -> PasteQueueSettings {
    PasteQueueSettings {
        enabled: value.enabled,
        insert_at_start: value.insert_at_start,
        end_behavior: match value.end_behavior {
            PasteQueueEndBehavior::Stop => QueueEndBehavior::Stop,
            PasteQueueEndBehavior::Loop => QueueEndBehavior::Loop,
            PasteQueueEndBehavior::KeepLast => QueueEndBehavior::KeepLast,
        },
        separator: match value.separator {
            PasteQueueSeparator::NewLine => QueueSeparator::NewLine,
            PasteQueueSeparator::BlankLine => QueueSeparator::BlankLine,
            PasteQueueSeparator::Tab => QueueSeparator::Tab,
            PasteQueueSeparator::Space => QueueSeparator::Space,
            PasteQueueSeparator::Comma => QueueSeparator::Comma,
            PasteQueueSeparator::CommaSpace => QueueSeparator::CommaSpace,
            PasteQueueSeparator::None => QueueSeparator::None,
            PasteQueueSeparator::Custom => QueueSeparator::Custom,
        },
        custom_separator: value.custom_separator,
        plain_text: value.plain_text,
        idle_stop_minutes: value.idle_stop_minutes,
        max_entries: value.max_entries,
        move_to_next_field: value.move_to_next_field,
        restore_when_finished: value.restore_when_finished,
    }
}

fn entry(value: ipc::QueueEntry) -> PasteQueueEntryView {
    PasteQueueEntryView {
        entry_id: value.entry_id,
        position_label: value.position_label,
        status: match value.status {
            ipc::QueueEntryStatus::Pending => PasteQueueEntryStatus::Pending,
            ipc::QueueEntryStatus::Pasted => PasteQueueEntryStatus::Pasted,
            ipc::QueueEntryStatus::Skipped => PasteQueueEntryStatus::Skipped,
            ipc::QueueEntryStatus::Unavailable => PasteQueueEntryStatus::Unavailable,
        },
        content_kind: match value.content_kind {
            ipc::QueueContentKind::Text => PasteQueueContentKind::Text,
            ipc::QueueContentKind::RichText => PasteQueueContentKind::RichText,
            ipc::QueueContentKind::Image => PasteQueueContentKind::Image,
            ipc::QueueContentKind::Files => PasteQueueContentKind::Files,
            ipc::QueueContentKind::Link => PasteQueueContentKind::Link,
        },
        preview: value.preview,
        masked: value.masked,
        edited: value.edited,
        source_app_bundle_id: value.source_app_bundle_id,
        source_app_name: value.source_app_name,
        source_app_icon_id: value.source_app_icon_id,
        added_via: match value.added_via {
            ipc::QueueAddedVia::Collect => PasteQueueAddedVia::Collect,
            ipc::QueueAddedVia::HistoryAdd => PasteQueueAddedVia::HistoryAdd,
            ipc::QueueAddedVia::SnippetAdd => PasteQueueAddedVia::SnippetAdd,
            ipc::QueueAddedVia::SelectionAction => PasteQueueAddedVia::SelectionAction,
            ipc::QueueAddedVia::Share => PasteQueueAddedVia::Share,
        },
        rendered_at_ms: value.rendered_at,
    }
}

fn state(value: ipc::PasteQueueState) -> PasteQueueView {
    PasteQueueView {
        revision: value.revision,
        collecting: value.collecting,
        phase: match value.phase {
            ipc::QueuePhase::Empty => PasteQueuePhase::Empty,
            ipc::QueuePhase::Active => PasteQueuePhase::Active,
            ipc::QueuePhase::Finished => PasteQueuePhase::Finished,
        },
        full: value.full,
        entries: value.entries.into_iter().map(entry).collect(),
        stale_since_ms: value.stale_since,
        undo_until_ms: value.undo_until,
        settings: preferences(value.settings),
    }
}

fn source(value: PasteQueueSourceInput) -> Result<ipc::QueueSource, RuntimeError> {
    let text = |via| {
        value
            .text
            .clone()
            .map(|text| ipc::QueueSource::Text { text, via })
            .ok_or_else(RuntimeError::internal)
    };
    match value.kind {
        PasteQueueSourceKind::Clip => Ok(ipc::QueueSource::Clip {
            id: value.id.clone().ok_or_else(RuntimeError::internal)?,
        }),
        PasteQueueSourceKind::Snippet => Ok(ipc::QueueSource::Snippet {
            id: value.id.clone().ok_or_else(RuntimeError::internal)?,
            values: template_values(value.values.clone()),
        }),
        PasteQueueSourceKind::SelectionText => text(ipc::QueueTextVia::SelectionAction),
        PasteQueueSourceKind::SharedText => text(ipc::QueueTextVia::Share),
    }
}

async fn queue_state(op: PasteQueueOp) -> Result<PasteQueueView, RuntimeError> {
    Ok(state(expect_data!(
        Method::PasteQueue { op },
        ResponseData::PasteQueue
    )))
}

pub async fn paste_queue_state() -> Result<PasteQueueView, RuntimeError> {
    queue_state(PasteQueueOp::Get).await
}

/// `append` keeps finished entries when collecting restarts after Finished.
pub async fn set_paste_queue_collecting(
    on: bool,
    append: bool,
) -> Result<PasteQueueView, RuntimeError> {
    queue_state(PasteQueueOp::SetCollecting {
        on,
        on_finished: if append {
            ipc::QueueRestart::Append
        } else {
            ipc::QueueRestart::StartFresh
        },
    })
    .await
}

pub async fn add_to_paste_queue(
    sources: Vec<PasteQueueSourceInput>,
) -> Result<PasteQueueView, RuntimeError> {
    let sources = sources
        .into_iter()
        .map(source)
        .collect::<Result<Vec<_>, _>>()?;
    queue_state(PasteQueueOp::Add { sources }).await
}

pub async fn remove_paste_queue_entries(
    entry_ids: Vec<String>,
) -> Result<PasteQueueView, RuntimeError> {
    queue_state(PasteQueueOp::Remove { entry_ids }).await
}

pub async fn remove_pasted_queue_entries() -> Result<PasteQueueView, RuntimeError> {
    queue_state(PasteQueueOp::RemovePasted).await
}

pub async fn clear_paste_queue() -> Result<PasteQueueView, RuntimeError> {
    queue_state(PasteQueueOp::Clear).await
}

pub async fn undo_clear_paste_queue() -> Result<PasteQueueView, RuntimeError> {
    queue_state(PasteQueueOp::UndoClear).await
}

pub async fn reverse_paste_queue() -> Result<PasteQueueView, RuntimeError> {
    queue_state(PasteQueueOp::Reverse).await
}

pub async fn move_paste_queue_entry(
    entry_id: String,
    to_index: u32,
) -> Result<PasteQueueView, RuntimeError> {
    queue_state(PasteQueueOp::Move { entry_id, to_index }).await
}

/// `None` drops the entry's local override.
pub async fn edit_paste_queue_entry(
    entry_id: String,
    text: Option<String>,
) -> Result<PasteQueueView, RuntimeError> {
    queue_state(PasteQueueOp::Edit { entry_id, text }).await
}

/// `entry_id` is required for [`PasteQueueTargetKind::Entry`].
pub async fn deliver_from_paste_queue(
    target: PasteQueueTargetKind,
    entry_id: Option<String>,
    plain_text: Option<bool>,
    mode: PasteQueueDeliveryMode,
) -> Result<PasteQueueDeliveryView, RuntimeError> {
    let target = match target {
        PasteQueueTargetKind::Next => ipc::QueueTarget::Next,
        PasteQueueTargetKind::All => ipc::QueueTarget::All,
        PasteQueueTargetKind::Entry => ipc::QueueTarget::Entry {
            id: entry_id.ok_or_else(RuntimeError::internal)?,
        },
    };
    let delivery = expect_data!(
        Method::PasteQueue {
            op: PasteQueueOp::Deliver {
                target,
                plain_text,
                mode: match mode {
                    PasteQueueDeliveryMode::Clipboard => ipc::QueueDeliveryMode::Clipboard,
                    PasteQueueDeliveryMode::Return => ipc::QueueDeliveryMode::Return,
                    PasteQueueDeliveryMode::CopyOnly => ipc::QueueDeliveryMode::CopyOnly,
                },
            },
        },
        ResponseData::QueueDelivery
    );
    Ok(PasteQueueDeliveryView {
        token: delivery.token,
        entry_ids: delivery.entry_ids,
        text: delivery.text,
        unavailable: delivery.notices.unavailable,
        finished: delivery.finished,
        nothing_to_paste: delivery.nothing_to_paste.map(|reason| match reason {
            ipc::NothingToPaste::Empty => PasteQueueNothingToPaste::Empty,
            ipc::NothingToPaste::Finished => PasteQueueNothingToPaste::Finished,
            ipc::NothingToPaste::Mixed => PasteQueueNothingToPaste::Mixed,
        }),
    })
}

pub async fn confirm_paste_queue_delivery(
    token: String,
    inserted: bool,
) -> Result<PasteQueueView, RuntimeError> {
    queue_state(PasteQueueOp::Confirm {
        token,
        outcome: if inserted {
            ipc::QueueDeliveryOutcome::Inserted
        } else {
            ipc::QueueDeliveryOutcome::Rejected
        },
    })
    .await
}

pub async fn step_back_paste_queue() -> Result<PasteQueueView, RuntimeError> {
    queue_state(PasteQueueOp::StepBack).await
}

pub async fn skip_paste_queue_entry() -> Result<PasteQueueView, RuntimeError> {
    queue_state(PasteQueueOp::Skip).await
}

pub async fn paste_queue_preferences() -> Result<PasteQueuePreferences, RuntimeError> {
    let applied = expect_data!(Method::GetConfig, ResponseData::Config);
    Ok(preferences(applied.config.paste_queue))
}

pub async fn set_paste_queue_preferences(
    value: PasteQueuePreferences,
) -> Result<PasteQueuePreferences, RuntimeError> {
    let applied = expect_data!(
        Method::SetConfig {
            patch: ConfigPatch {
                paste_queue: Some(settings(value)),
                ..ConfigPatch::default()
            },
        },
        ResponseData::Config
    );
    Ok(preferences(applied.config.paste_queue))
}
