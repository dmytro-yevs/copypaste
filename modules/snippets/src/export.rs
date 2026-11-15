//! Snippet export.
//!
//! Writes live snippets of the chosen scope, with the collections they are
//! filed in, as a bundle. Bodies are read one at a time and streamed through
//! the encryption, so memory stays at one snippet plus one chunk. Concealed
//! snippets go into an unencrypted bundle only when the caller confirmed it.

use std::collections::HashSet;
use std::io::{self, BufWriter, Write};
use std::path::Path;

use crate::bundle::{
    check_passphrase, BundleCollection, BundleSnippet, SealWriter, TransferError, FORMAT, VERSION,
};
use crate::library::{Library, Scope};
use crate::model;

/// What an export covers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExportScope {
    All,
    Collection(String),
    Snippets(Vec<String>),
}

/// Writes the snippets `scope` names to `path`, sealed with `passphrase`
/// when one is given; returns how many were written.
pub(crate) fn export(
    library: &Library,
    path: &Path,
    scope: &ExportScope,
    passphrase: Option<&str>,
    include_concealed: bool,
) -> Result<u32, TransferError> {
    if let Some(passphrase) = passphrase {
        check_passphrase(passphrase)?;
    }
    let concealed = passphrase.is_some() || include_concealed;
    let chosen: Vec<(String, Option<String>)> = chosen(library, scope)
        .into_iter()
        .filter(|id| concealed || library.meta(id).is_some_and(|meta| !meta.concealed))
        .map(|id| {
            let filed = library
                .meta(&id)
                .and_then(|meta| library.filed_in(meta))
                .map(str::to_owned);
            (id, filed)
        })
        .collect();
    let mut filed: HashSet<&str> = chosen
        .iter()
        .filter_map(|(_, collection)| collection.as_deref())
        .collect();
    if let ExportScope::Collection(id) = scope {
        filed.insert(id);
    }
    let collections: Vec<BundleCollection> = library
        .collections()
        .into_iter()
        .filter(|view| filed.contains(view.id.as_str()))
        .map(|view| BundleCollection {
            id: view.id.clone(),
            name: view.collection.name.clone(),
            icon: Some(view.collection.icon.clone()),
            accent: Some(view.collection.accent.clone()),
        })
        .collect();

    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let file = std::fs::File::create(path)?;
    let written = (|| {
        let out = BufWriter::new(&file);
        match passphrase {
            Some(passphrase) => {
                let mut sealed = SealWriter::new(out, passphrase)?;
                let written = write_bundle(&mut sealed, library, &collections, &chosen)?;
                sealed.finish()?.flush()?;
                Ok::<_, TransferError>(written)
            }
            None => {
                let mut out = out;
                let written = write_bundle(&mut out, library, &collections, &chosen)?;
                out.flush()?;
                Ok(written)
            }
        }
    })();
    match written {
        Ok(written) => {
            file.sync_all()?;
            Ok(written)
        }
        Err(error) => {
            drop(file);
            let _ = std::fs::remove_file(path);
            Err(error)
        }
    }
}

/// Live snippets in display order, without their bodies.
fn chosen(library: &Library, scope: &ExportScope) -> Vec<String> {
    match scope {
        ExportScope::All => library
            .page(&Scope::All)
            .into_iter()
            .map(|(id, _)| id.clone())
            .collect(),
        ExportScope::Collection(id) => library
            .page(&Scope::Collection(id.clone()))
            .into_iter()
            .map(|(id, _)| id.clone())
            .collect(),
        ExportScope::Snippets(ids) => {
            let mut seen = HashSet::new();
            ids.iter()
                .filter(|id| library.live(id).is_ok() && seen.insert(*id))
                .cloned()
                .collect()
        }
    }
}

fn write_bundle(
    out: &mut impl Write,
    library: &Library,
    collections: &[BundleCollection],
    chosen: &[(String, Option<String>)],
) -> Result<u32, TransferError> {
    out.write_all(b"{\"format\":")?;
    serde_json::to_writer(&mut *out, FORMAT).map_err(io::Error::from)?;
    write!(
        out,
        ",\"version\":{VERSION},\"exported_at\":{},\"collections\":",
        model::now_ms()
    )?;
    serde_json::to_writer(&mut *out, collections).map_err(io::Error::from)?;
    out.write_all(b",\"snippets\":[")?;
    let mut written = 0_u32;
    for (id, collection_id) in chosen {
        let (Some(meta), Some(body)) = (library.meta(id), library.body(id)?) else {
            continue;
        };
        if written > 0 {
            out.write_all(b",")?;
        }
        let entry = BundleSnippet {
            id: id.clone(),
            collection_id: collection_id.clone(),
            title: meta.title.clone(),
            abbreviation: meta.abbreviation.clone(),
            template: meta.template,
            pinned: meta.pinned,
            concealed: meta.concealed,
            body_text: body.content().to_owned(),
            body_rich: body.rich_html.clone(),
        };
        serde_json::to_writer(&mut *out, &entry).map_err(io::Error::from)?;
        written += 1;
    }
    out.write_all(b"]}")?;
    Ok(written)
}
