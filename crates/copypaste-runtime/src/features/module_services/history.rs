//! History reads for modules, within a module scope and with secrets
//! redacted from every text they return.

use copypaste_core::history_actions::{self, CopyOptions};
use copypaste_core::history_error::HistoryError;
use copypaste_core::history_item::ItemOrigin;
use copypaste_core::{HistoryCursor, IntegrationKinds, IntegrationView, StoredItem};
use copypaste_ipc::{
    content_type, ContentClass, HistoryQuery, HistorySort, Method, ResponseData, SemanticKind,
    MAX_PAGE_CONTENT_BYTES,
};
use copypaste_module_sdk::{
    DeviceName, HistoryContent, HistoryContentKind, HistoryEntry, HistoryFacet, HistoryFacets,
    HistoryItem, HistoryPage, HistoryScope, HostError, HISTORY_PREVIEW_CHARS,
    HISTORY_TEXT_CHUNK_CHARS, MAX_HISTORY_PAGE,
};

use super::redact::{bounded, redact};
use super::{invalid, not_found, storage, unavailable};
use crate::Runtime;

/// Longest edge of an image preview handed to a module.
const IMAGE_EDGE: u32 = 1568;
const MSG_CURSOR: &str = "The cursor does not belong to this query.";
const MSG_OFFSET: &str = "The offset is past the end of the text.";

/// The filters of a history query, beside its scope.
#[derive(Debug, Clone, Default)]
pub struct ModuleHistoryFilters {
    pub search: Option<String>,
    pub kind: Option<HistoryContentKind>,
    /// A source app id from the scope's facets.
    pub source_app: Option<String>,
    /// A device id from the scope's facets.
    pub device: Option<String>,
    pub since_ms: Option<i64>,
    pub until_ms: Option<i64>,
    pub pinned_only: bool,
}

fn kinds_of(kinds: &[HistoryContentKind]) -> IntegrationKinds {
    let has = |kind| kinds.contains(&kind);
    IntegrationKinds {
        text: has(HistoryContentKind::Text),
        link: has(HistoryContentKind::Link),
        rich_text: has(HistoryContentKind::RichText),
        image: has(HistoryContentKind::Image),
        file: has(HistoryContentKind::File),
    }
}

fn shows(kinds: IntegrationKinds, kind: HistoryContentKind) -> bool {
    match kind {
        HistoryContentKind::Text => kinds.text,
        HistoryContentKind::Link => kinds.link,
        HistoryContentKind::RichText => kinds.rich_text,
        HistoryContentKind::Image => kinds.image,
        HistoryContentKind::File => kinds.file,
    }
}

fn kind_of(content_type: &str, semantic_kind: Option<SemanticKind>) -> Option<HistoryContentKind> {
    match content_type::classify(content_type) {
        ContentClass::Image => Some(HistoryContentKind::Image),
        ContentClass::File => Some(HistoryContentKind::File),
        ContentClass::Text if semantic_kind == Some(SemanticKind::Credentials) => None,
        ContentClass::Text if content_type != content_type::TEXT => {
            Some(HistoryContentKind::RichText)
        }
        ContentClass::Text if semantic_kind == Some(SemanticKind::Link) => {
            Some(HistoryContentKind::Link)
        }
        ContentClass::Text => Some(HistoryContentKind::Text),
        ContentClass::Other => None,
    }
}

fn entry(
    row: &StoredItem,
    origin: &ItemOrigin,
    kind: HistoryContentKind,
    preview: String,
    truncated: bool,
) -> HistoryEntry {
    HistoryEntry {
        id: row.id.clone(),
        kind,
        preview,
        truncated,
        pinned: row.pinned,
        captured_at_ms: row.created_at,
        source_app_id: row.app_bundle_id.clone(),
        source_app_name: row.app_name.clone(),
        device_id: origin.device_id.clone(),
        device_name: origin.device_name.clone(),
    }
}

/// A listed text preview, or `None` when the text is mostly secret.
fn text_preview(text: &str, cut_before: bool) -> Option<(String, bool)> {
    let redacted = redact(text);
    (!redacted.mostly_secret()).then(|| bounded(&redacted.text, HISTORY_PREVIEW_CHARS, cut_before))
}

impl Runtime {
    /// What `scope` shows after the host's own exclusions, which include the
    /// apps excluded from capture.
    fn integration_view(&self, scope: &HistoryScope) -> IntegrationView {
        let mut excluded_source_apps = scope.excluded_source_apps.clone();
        excluded_source_apps.extend(self.settings.get().excluded_app_bundle_ids.iter().cloned());
        IntegrationView {
            local_device_id: self.meta.device_id().to_owned(),
            include_other_devices: scope.include_other_devices,
            excluded_devices: scope.excluded_devices.clone(),
            excluded_source_apps,
            kinds: kinds_of(&scope.content_kinds),
            lookback_days: scope.lookback_days,
        }
    }

    pub(super) fn visible(
        &self,
        scope: &HistoryScope,
        id: &str,
    ) -> Result<IntegrationView, HostError> {
        let view = self.integration_view(scope);
        if self.store.integration_visible(id, &view).map_err(storage)? {
            Ok(view)
        } else {
            Err(not_found())
        }
    }

    /// A page of visible items, newest first or by relevance to `search`.
    /// Mostly secret texts are left out, so a page may hold fewer items.
    pub fn module_history_query(
        &self,
        scope: &HistoryScope,
        filters: &ModuleHistoryFilters,
        limit: u32,
        cursor: Option<&str>,
    ) -> Result<HistoryPage, HostError> {
        let mut view = self.integration_view(scope);
        if let Some(kind) = filters.kind {
            view.kinds = IntegrationKinds {
                text: kind == HistoryContentKind::Text && view.kinds.text,
                link: kind == HistoryContentKind::Link && view.kinds.link,
                rich_text: kind == HistoryContentKind::RichText && view.kinds.rich_text,
                image: kind == HistoryContentKind::Image && view.kinds.image,
                file: kind == HistoryContentKind::File && view.kinds.file,
            };
        }
        let search = filters
            .search
            .as_deref()
            .map(str::trim)
            .filter(|search| !search.is_empty());
        let query = HistoryQuery {
            search: search.map(str::to_owned),
            sort: if search.is_some() {
                HistorySort::Relevance
            } else {
                HistorySort::Newest
            },
            pinned_only: filters.pinned_only,
            origin_device_id: filters.device.clone(),
            source_app_bundle_id: filters.source_app.clone(),
            captured_from_ms: filters.since_ms,
            captured_before_ms: filters.until_ms,
            ..HistoryQuery::default()
        };
        let after = cursor
            .map(|token| HistoryCursor::parse_scoped(token, &query, &view))
            .transpose()
            .map_err(|_| invalid(MSG_CURSOR))?;
        let page = self
            .store
            .query_integration_history(
                &query,
                &view,
                after.as_ref(),
                limit.clamp(1, MAX_HISTORY_PAGE),
                MAX_PAGE_CONTENT_BYTES,
                (&self.keyring.item_key(), &self.keyring.preview_key()),
            )
            .map_err(storage)?;
        let rows: Vec<StoredItem> = page.items.iter().map(|entry| entry.row.clone()).collect();
        let origins = self.meta.origins_for(&rows);
        let mut items = Vec::with_capacity(rows.len());
        for listed in page.items {
            let (Some(preview), Some(origin)) = (listed.preview, origins.get(&listed.row.id))
            else {
                continue;
            };
            let item = preview.into_item(&listed.row, origin, self.meta.device_id());
            let Some(kind) = kind_of(&item.content_type, item.semantic_kind)
                .filter(|kind| shows(view.kinds, *kind))
            else {
                continue;
            };
            let (preview, truncated) = match kind {
                HistoryContentKind::Image | HistoryContentKind::File => (String::new(), false),
                _ => match text_preview(&item.content, item.truncated) {
                    Some(preview) => preview,
                    None => continue,
                },
            };
            items.push(entry(&listed.row, origin, kind, preview, truncated));
        }
        Ok(HistoryPage {
            items,
            next_cursor: page.next.map(|cursor| cursor.token()),
        })
    }

    /// One visible item. Text comes in chunks of the redacted text from
    /// `offset`; an item that is mostly secret is not found.
    pub fn module_history_get(
        &self,
        scope: &HistoryScope,
        id: &str,
        offset: u64,
    ) -> Result<HistoryItem, HostError> {
        let view = self.visible(scope, id)?;
        let row = self.store.get(id).map_err(storage)?.ok_or_else(not_found)?;
        let origin = self.meta.origin_of(&row);
        match content_type::classify(&row.content_type) {
            ContentClass::Image => {
                if !view.kinds.image {
                    return Err(not_found());
                }
                Ok(HistoryItem {
                    entry: entry(
                        &row,
                        &origin,
                        HistoryContentKind::Image,
                        String::new(),
                        false,
                    ),
                    content: self.image_content(id),
                })
            }
            ContentClass::File => {
                if !view.kinds.file {
                    return Err(not_found());
                }
                let entry = entry(
                    &row,
                    &origin,
                    HistoryContentKind::File,
                    String::new(),
                    false,
                );
                let (item, _) = self.to_wire(row).map_err(|error| unavailable(&error))?;
                let file = item.file_details;
                let local = item.origin_device_id == self.meta.device_id();
                let path = file
                    .as_ref()
                    .filter(|file| local && file.source_available)
                    .and_then(|file| file.source_reference.clone());
                Ok(HistoryItem {
                    entry,
                    content: HistoryContent::File {
                        available_on: path.is_none().then_some(item.origin_device_name).flatten(),
                        path,
                        name: file.as_ref().and_then(|file| file.filename.clone()),
                        mime_type: file.as_ref().and_then(|file| file.mime_type.clone()),
                        size_bytes: file.as_ref().map(|file| file.size_bytes),
                    },
                })
            }
            ContentClass::Text => self.text_item(&view, row, &origin, offset),
            ContentClass::Other => Err(not_found()),
        }
    }

    fn text_item(
        &self,
        view: &IntegrationView,
        row: StoredItem,
        origin: &ItemOrigin,
        offset: u64,
    ) -> Result<HistoryItem, HostError> {
        let key = self.keyring.item_key();
        let opened = history_actions::open(&self.store, &key, &row.id).map_err(history)?;
        let prepared = history_actions::prepare_copy(
            &self.store,
            &key,
            opened,
            CopyOptions {
                origin,
                local_device_id: self.meta.device_id(),
                plain_text: true,
                allow_content_uri: false,
                max_file_size_bytes: 0,
            },
        )
        .map_err(history)?;
        let kind = kind_of(&prepared.item.content_type, prepared.item.semantic_kind)
            .filter(|kind| shows(view.kinds, *kind))
            .ok_or_else(not_found)?;
        let redacted = redact(prepared.payload().plain_text().ok_or_else(not_found)?);
        drop(prepared);
        if redacted.mostly_secret() {
            return Err(not_found());
        }
        let total = redacted.text.chars().count();
        let start = usize::try_from(offset).unwrap_or(usize::MAX);
        if start > total {
            return Err(invalid(MSG_OFFSET));
        }
        let text: String = redacted
            .text
            .chars()
            .skip(start)
            .take(HISTORY_TEXT_CHUNK_CHARS)
            .collect();
        let end = start + text.chars().count();
        let (preview, truncated) = bounded(&redacted.text, HISTORY_PREVIEW_CHARS, false);
        Ok(HistoryItem {
            entry: entry(&row, origin, kind, preview, truncated),
            content: HistoryContent::Text {
                text,
                offset,
                total_chars: total as u64,
                next_offset: (end < total).then_some(end as u64),
            },
        })
    }

    /// A bounded PNG preview through the shared preview admission.
    fn image_content(&self, id: &str) -> HistoryContent {
        let response = self.request_blocking(
            0,
            Method::ImagePreview {
                id: id.to_owned(),
                max_edge: Some(IMAGE_EDGE),
                bounds: None,
            },
        );
        match response.data {
            Some(ResponseData::ImagePreview(preview)) if response.ok => HistoryContent::Image {
                width: Some(preview.width),
                height: Some(preview.height),
                png_base64: Some(preview.png_base64),
            },
            _ => HistoryContent::Image {
                width: None,
                height: None,
                png_base64: None,
            },
        }
    }

    /// Source apps and devices that have an item visible in `scope`.
    pub fn module_history_facets(&self, scope: &HistoryScope) -> Result<HistoryFacets, HostError> {
        let facets = self
            .store
            .integration_facets(
                &self.integration_view(scope),
                &self.meta.device_name(),
                self.meta.device_class(),
            )
            .map_err(storage)?;
        Ok(HistoryFacets {
            source_apps: facets
                .source_apps
                .into_iter()
                .map(|facet| HistoryFacet {
                    id: facet.id,
                    label: facet.label,
                })
                .collect(),
            devices: facets
                .origin_devices
                .into_iter()
                .map(|facet| HistoryFacet {
                    id: facet.id,
                    label: facet.label,
                })
                .collect(),
        })
    }

    /// This device first, then every device it knows a name for.
    pub fn module_device_names(&self) -> Result<Vec<DeviceName>, HostError> {
        let local = self.meta.device_id();
        let mut names = vec![DeviceName {
            id: local.to_owned(),
            name: self.meta.device_name(),
            local: true,
        }];
        names.extend(
            self.store
                .known_device_names()
                .map_err(storage)?
                .into_iter()
                .filter(|(id, _)| id != local)
                .map(|(id, name)| DeviceName {
                    id,
                    name,
                    local: false,
                }),
        );
        Ok(names)
    }
}

fn history(error: HistoryError) -> HostError {
    match error {
        HistoryError::NotFound | HistoryError::Unsupported | HistoryError::InvalidRichText => {
            not_found()
        }
        HistoryError::ContentTooLarge => invalid("The item is too large to read."),
        error => unavailable(&error),
    }
}
