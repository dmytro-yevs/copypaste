//! Moves the library the app kept before snippets became a module.
//!
//! On first use on a device the module pages the legacy library and writes
//! each collection and snippet only when its record is absent, so running
//! on several devices, or again after an interruption, never overwrites
//! newer module data. Local usage counters move into local state. A flag in
//! local state marks the device done.

use std::collections::BTreeMap;

use copypaste_module_sdk::{HostErrorCode, LegacySnippetPage, ModuleHostRequest, MAX_LEGACY_PAGE};

use crate::conflict::MAX_ANCESTORS;
use crate::host;
use crate::library::{Library, Result};
use crate::model::{self, Body, Collection, Meta, BODY, COLLECTION, META};

const STATE_MIGRATED: &str = "legacy_migrated";

/// Runs the migration once per device; a host without the legacy library
/// leaves the flag unset so a later host can still offer it.
pub(crate) fn run(library: &mut Library) -> Result<()> {
    let state = host::read_state(library.host(), &[STATE_MIGRATED.to_owned()])?;
    if state.get(STATE_MIGRATED).cloned().flatten().is_some() {
        return Ok(());
    }
    let mut after = None;
    let mut wrote = false;
    loop {
        let page: LegacySnippetPage = match host::request(
            library.host(),
            ModuleHostRequest::LegacySnippets {
                after: after.take(),
                limit: MAX_LEGACY_PAGE,
            },
        ) {
            Ok(page) => page,
            Err(error)
                if matches!(
                    error.code,
                    HostErrorCode::Unsupported | HostErrorCode::Denied
                ) =>
            {
                return Ok(());
            }
            Err(error) => return Err(error.into()),
        };
        for legacy in &page.collections {
            let id = model::record_id(&legacy.id);
            if library.has_collection(&id) {
                continue;
            }
            wrote |= library.put_absent(
                &format!("{COLLECTION}{id}"),
                &Collection {
                    name: legacy.name.clone(),
                    icon: legacy.icon.clone(),
                    accent: legacy.accent.clone(),
                    order_key: legacy.order_key.clone(),
                    deleted_at_ms: legacy.deleted_at_ms,
                },
            )?;
        }
        for legacy in &page.snippets {
            let id = model::record_id(&legacy.id);
            if legacy.use_count > 0 || legacy.last_used_at_ms.is_some() {
                library.merge_usage(&id, legacy.use_count, legacy.last_used_at_ms)?;
            }
            if library.known(&id) {
                continue;
            }
            let body = Body::new(
                legacy.revision.clone(),
                legacy
                    .ancestors
                    .iter()
                    .take(MAX_ANCESTORS)
                    .cloned()
                    .collect(),
                legacy.text.clone(),
                legacy.rich_html.clone(),
                legacy.concealed,
            );
            wrote |= library.put_absent(&format!("{BODY}{id}"), &body)?;
            wrote |= library.put_absent(
                &format!("{META}{id}"),
                &Meta {
                    title: legacy.title.clone(),
                    abbreviation: legacy.abbreviation.clone(),
                    collection_id: legacy.collection_id.as_deref().map(model::record_id),
                    template: legacy.template,
                    pinned: legacy.pinned,
                    pin_order: legacy.pin_order,
                    order_key: legacy.order_key.clone(),
                    concealed: legacy.concealed,
                    deleted_at_ms: legacy.deleted_at_ms,
                    purged: false,
                    created_at_ms: legacy.created_at_ms,
                    created_device: legacy.created_device.clone(),
                    preview: model::preview(&legacy.text, legacy.concealed),
                },
            )?;
        }
        match page.next {
            Some(next) => after = Some(next),
            None => break,
        }
    }
    library.write_all_usage()?;
    host::write_state(
        library.host(),
        BTreeMap::from([(STATE_MIGRATED.to_owned(), "1".to_owned())]),
    )?;
    if wrote {
        library.invalidate();
    }
    Ok(())
}
