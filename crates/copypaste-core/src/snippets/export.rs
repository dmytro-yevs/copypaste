//! Snippet export.
//!
//! Writes live snippets of the chosen scope, with the collections they are
//! filed in, as a bundle. Bodies are read one at a time and streamed through
//! the encryption, so memory stays at one snippet plus one chunk. The file
//! appears at its destination only once complete. Concealed snippets go into
//! an unencrypted bundle only when the caller confirmed it.

use std::collections::HashSet;
use std::io::{self, BufWriter, Write};
use std::path::Path;

use copypaste_ipc::snippets::ExportScope;

use super::bundle::{
    check_passphrase, BundleCollection, BundleSnippet, SealWriter, TransferError, FORMAT, VERSION,
};
use crate::storage::snippets::{SnippetScope, SnippetSummary};
use crate::Store;

/// Writes the snippets `scope` names to `path`, sealed with `passphrase`
/// when one is given; returns how many were written.
pub fn export(
    store: &Store,
    path: &Path,
    scope: &ExportScope,
    passphrase: Option<&str>,
    include_concealed: bool,
) -> Result<u32, TransferError> {
    if let Some(passphrase) = passphrase {
        check_passphrase(passphrase)?;
    }
    let concealed = passphrase.is_some() || include_concealed;
    let chosen: Vec<(String, Option<String>)> = chosen(store, scope)?
        .into_iter()
        .filter(|summary| concealed || !summary.concealed)
        .map(|summary| (summary.id, summary.collection_id))
        .collect();
    let mut filed: HashSet<&str> = chosen
        .iter()
        .filter_map(|(_, collection)| collection.as_deref())
        .collect();
    if let ExportScope::Collections { ids } = scope {
        filed.extend(ids.iter().map(String::as_str));
    }
    let collections: Vec<BundleCollection> = store
        .snippet_collections()?
        .into_iter()
        .filter(|collection| filed.contains(collection.id.as_str()))
        .map(|collection| BundleCollection {
            id: collection.id.clone(),
            name: collection.name.clone(),
            icon: Some(collection.icon.clone()),
            accent: Some(collection.accent.clone()),
        })
        .collect();

    let dir = path
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut file = tempfile::Builder::new()
        .prefix(".copypaste-")
        .tempfile_in(dir)?;
    let written = {
        let out = BufWriter::new(file.as_file_mut());
        match passphrase {
            Some(passphrase) => {
                let mut sealed = SealWriter::new(out, passphrase)?;
                let written = write_bundle(&mut sealed, store, &collections, &chosen)?;
                sealed.finish()?.flush()?;
                written
            }
            None => {
                let mut out = out;
                let written = write_bundle(&mut out, store, &collections, &chosen)?;
                out.flush()?;
                written
            }
        }
    };
    file.as_file().sync_all()?;
    file.persist(path).map_err(|failed| failed.error)?;
    Ok(written)
}

/// Live snippets in display order, without their bodies.
fn chosen(store: &Store, scope: &ExportScope) -> Result<Vec<SnippetSummary>, TransferError> {
    let mut seen = HashSet::new();
    let mut chosen = Vec::new();
    let mut page_through = |scope: SnippetScope| -> Result<(), TransferError> {
        let mut cursor = None;
        loop {
            let page = store.snippet_page(&scope, cursor.as_deref(), copypaste_ipc::MAX_PAGE)?;
            for summary in page.snippets {
                if seen.insert(summary.id.clone()) {
                    chosen.push(summary);
                }
            }
            match page.next {
                Some(next) => cursor = Some(next),
                None => return Ok(()),
            }
        }
    };
    match scope {
        ExportScope::All => page_through(SnippetScope::All)?,
        ExportScope::Collections { ids } => {
            for id in ids {
                page_through(SnippetScope::Collection(id.clone()))?;
            }
        }
        ExportScope::Snippets { ids } => {
            for id in ids {
                if let Some(detail) = store.snippet(id)? {
                    if detail.snippet.deleted_at.is_none() && seen.insert(id.clone()) {
                        chosen.push(detail.snippet);
                    }
                }
            }
        }
    }
    Ok(chosen)
}

fn write_bundle(
    out: &mut impl Write,
    store: &Store,
    collections: &[BundleCollection],
    chosen: &[(String, Option<String>)],
) -> Result<u32, TransferError> {
    out.write_all(b"{\"format\":")?;
    serde_json::to_writer(&mut *out, FORMAT).map_err(io::Error::from)?;
    write!(
        out,
        ",\"version\":{VERSION},\"exported_at\":{},\"collections\":",
        crate::now_ms()
    )?;
    serde_json::to_writer(&mut *out, collections).map_err(io::Error::from)?;
    out.write_all(b",\"snippets\":[")?;
    let mut written = 0_u32;
    for (id, collection_id) in chosen {
        let Some(detail) = store.snippet(id)? else {
            continue;
        };
        if written > 0 {
            out.write_all(b",")?;
        }
        let snippet = &detail.snippet;
        let entry = BundleSnippet {
            id: snippet.id.clone(),
            collection_id: collection_id.clone(),
            title: snippet.title.clone(),
            abbreviation: snippet.abbreviation.clone(),
            template: snippet.template,
            pinned: snippet.pinned,
            concealed: snippet.concealed,
            body_text: detail.body_text.clone(),
            body_rich: detail.body_rich.clone(),
        };
        serde_json::to_writer(&mut *out, &entry).map_err(io::Error::from)?;
        written += 1;
    }
    out.write_all(b"]}")?;
    Ok(written)
}
