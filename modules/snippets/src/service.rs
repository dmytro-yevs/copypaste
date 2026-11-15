//! The `snippets` host service this module provides to other modules (AI
//! tools): search, get, render and save. Snippets served are live and never
//! concealed; the host redacts secrets in replies as well.

use std::collections::{BTreeMap, HashSet};

use copypaste_module_sdk::{
    HostError, HostErrorCode, ModuleHostRequest, RenderedSnippet, SavedRecord, ServiceReply,
    Snippet, SnippetSummary,
};

use crate::library::{Draft, Library, LibraryError, Scope};
use crate::render::{self, Clipboard, Templates};

/// Characters of one search preview.
const PREVIEW_CHARS: usize = 200;
const MSG_SNIPPET: &str = "The snippet does not exist or is not available to modules.";
const MSG_CLIPBOARD: &str =
    "This snippet inserts clipboard content, which CopyPaste does not share with modules. \
     Use a snippet without clipboard placeholders.";
const MSG_ABBREVIATION_TAKEN: &str = "That abbreviation is already used by another snippet.";
const MSG_EMPTY: &str = "The text must not be empty.";
const MSG_REQUEST: &str = "The snippet service request is not valid.";

fn not_found() -> HostError {
    HostError::new(HostErrorCode::NotFound, MSG_SNIPPET)
}

fn invalid(message: &str) -> HostError {
    HostError::new(HostErrorCode::Invalid, message)
}

fn host_error(error: LibraryError) -> HostError {
    match error {
        LibraryError::Host(error) => error,
        LibraryError::NotFound => not_found(),
        LibraryError::AbbreviationTaken { .. } => invalid(MSG_ABBREVIATION_TAKEN),
        other => invalid(&other.message()),
    }
}

/// Answers one request as the `Data` of the `serve` command.
pub(crate) fn serve(
    library: &mut Library,
    templates: &Templates,
    locale: &str,
    request: &str,
) -> serde_json::Value {
    let reply = serde_json::from_str::<ModuleHostRequest>(request)
        .map_err(|_| invalid(MSG_REQUEST))
        .and_then(|request| {
            request.validate()?;
            answer(library, templates, locale, request)
        });
    let reply = match reply {
        Ok(reply) => ServiceReply::Ok { reply },
        Err(error) => ServiceReply::Error { error },
    };
    serde_json::to_value(reply).unwrap_or_default()
}

fn json<T: serde::Serialize>(value: T) -> Result<serde_json::Value, HostError> {
    serde_json::to_value(value).map_err(|_| HostError::new(HostErrorCode::Unavailable, MSG_REQUEST))
}

fn answer(
    library: &mut Library,
    templates: &Templates,
    locale: &str,
    request: ModuleHostRequest,
) -> Result<serde_json::Value, HostError> {
    match request {
        ModuleHostRequest::SnippetSearch { query, limit } => {
            json(search(library, &query, limit as usize)?)
        }
        ModuleHostRequest::SnippetGet { id } => json(get(library, templates, locale, &id)?),
        ModuleHostRequest::SnippetRender { id, values } => {
            json(render_snippet(library, templates, locale, &id, &values)?)
        }
        ModuleHostRequest::SnippetSave {
            title,
            body,
            abbreviation,
        } => json(save(library, title, body, abbreviation)?),
        _ => Err(HostError::new(HostErrorCode::Unsupported, MSG_REQUEST)),
    }
}

/// Matching snippets, or the first of the library for an empty query.
fn search(
    library: &mut Library,
    query: &str,
    limit: usize,
) -> Result<Vec<SnippetSummary>, HostError> {
    let ids: Vec<String> = if query.trim().is_empty() {
        library
            .page(&Scope::All)
            .into_iter()
            .filter(|(_, meta)| !meta.concealed)
            .take(limit)
            .map(|(id, _)| id.clone())
            .collect()
    } else {
        library
            .quick_search(query, limit)
            .map_err(host_error)?
            .into_iter()
            .map(|hit| hit.id)
            .collect()
    };
    Ok(ids
        .into_iter()
        .filter_map(|id| {
            let meta = library.live(&id).ok().filter(|meta| !meta.concealed)?;
            Some(SnippetSummary {
                title: meta.title.clone(),
                abbreviation: meta.abbreviation.clone(),
                preview: meta.preview.chars().take(PREVIEW_CHARS).collect(),
                id,
            })
        })
        .collect())
}

/// A live snippet a module may read: never a concealed or deleted one.
fn readable(library: &Library, id: &str) -> Result<crate::model::Body, HostError> {
    library
        .live(id)
        .ok()
        .filter(|meta| !meta.concealed)
        .ok_or_else(not_found)?;
    library.body(id).map_err(host_error)?.ok_or_else(not_found)
}

fn get(
    library: &Library,
    templates: &Templates,
    locale: &str,
    id: &str,
) -> Result<Snippet, HostError> {
    let body = readable(library, id)?;
    let meta = library.live(id).map_err(host_error)?;
    let values = BTreeMap::new();
    let placeholders = render::stored(id, library, &body, &values, Clipboard::None, locale)
        .map(|expansion| expansion.fields(library, templates))
        .unwrap_or_default()
        .into_iter()
        .map(|field| field.name)
        .collect();
    Ok(Snippet {
        id: id.to_owned(),
        title: meta.title.clone(),
        body: body.content().to_owned(),
        abbreviation: meta.abbreviation.clone(),
        placeholders,
    })
}

/// Expands a snippet with `values`. A module cannot receive clipboard
/// content, so a snippet with clipboard placeholders is refused rather than
/// rendered with them empty.
fn render_snippet(
    library: &Library,
    templates: &Templates,
    locale: &str,
    id: &str,
    values: &BTreeMap<String, String>,
) -> Result<RenderedSnippet, HostError> {
    let body = readable(library, id)?;
    let no_values = BTreeMap::new();
    let expansion = render::stored(id, library, &body, &no_values, Clipboard::None, locale)
        .ok_or_else(not_found)?;
    let known: HashSet<String> = expansion
        .fields(library, templates)
        .into_iter()
        .map(|field| field.name)
        .collect();
    let unknown: Vec<&str> = values
        .keys()
        .map(String::as_str)
        .filter(|name| !known.contains(*name))
        .collect();
    if !unknown.is_empty() {
        return Err(invalid(&format!("Unknown values: {}", unknown.join(", "))));
    }
    let expansion = render::stored(id, library, &body, values, Clipboard::None, locale)
        .ok_or_else(not_found)?;
    let rendered = expansion
        .render(library, templates)
        .map_err(|error| HostError::new(HostErrorCode::Unavailable, error.to_string()))?;
    if rendered.secret {
        return Err(not_found());
    }
    if rendered.reads_clipboard() {
        return Err(invalid(MSG_CLIPBOARD));
    }
    let missing = rendered.missing_fields();
    if !missing.is_empty() {
        return Err(invalid(&format!(
            "Fill in these values: {}.",
            missing.join(", ")
        )));
    }
    Ok(RenderedSnippet {
        text: rendered.text,
    })
}

fn save(
    library: &mut Library,
    title: String,
    body: String,
    abbreviation: Option<String>,
) -> Result<SavedRecord, HostError> {
    if body.is_empty() {
        return Err(invalid(MSG_EMPTY));
    }
    let draft = Draft {
        title,
        abbreviation: abbreviation.filter(|value| !value.trim().is_empty()),
        body_text: body,
        ..Draft::default()
    };
    let id = library.save(&draft, None).map_err(host_error)?;
    Ok(SavedRecord { id })
}
