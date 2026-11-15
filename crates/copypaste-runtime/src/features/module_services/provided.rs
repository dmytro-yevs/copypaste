//! Snippet requests answered by the module that provides the snippets
//! service. Its replies are redacted like every other text a module reads.

use copypaste_module_sdk::{
    HostError, HostErrorCode, ModuleHostRequest, ModuleService, RenderedSnippet, Snippet,
    SnippetSummary,
};
use serde::de::DeserializeOwned;

use super::redact::{bounded, redact};
use super::unavailable;
use crate::Runtime;

/// Characters of one snippet search preview.
const PREVIEW_CHARS: usize = 200;
const MSG_SNIPPET: &str = "The snippet does not exist or is not available to modules.";

fn hidden() -> HostError {
    HostError::new(HostErrorCode::NotFound, MSG_SNIPPET)
}

impl Runtime {
    /// `None` when no other enabled module provides snippets.
    pub(super) fn provided<T: DeserializeOwned>(
        &self,
        caller: &str,
        request: &ModuleHostRequest,
    ) -> Option<Result<T, HostError>> {
        let reply = self
            .modules
            .serve_service(caller, ModuleService::Snippets, request)?;
        Some(reply.and_then(|value| serde_json::from_value(value).map_err(|e| unavailable(&e))))
    }
}

pub(super) fn summaries(found: Vec<SnippetSummary>) -> Vec<SnippetSummary> {
    found
        .into_iter()
        .filter_map(|summary| {
            let redacted = redact(&summary.preview);
            (!redacted.mostly_secret()).then(|| SnippetSummary {
                preview: bounded(&redacted.text, PREVIEW_CHARS, false).0,
                ..summary
            })
        })
        .collect()
}

pub(super) fn snippet(snippet: Snippet) -> Result<Snippet, HostError> {
    let redacted = redact(&snippet.body);
    if redacted.mostly_secret() {
        return Err(hidden());
    }
    Ok(Snippet {
        body: redacted.text,
        ..snippet
    })
}

pub(super) fn rendered(rendered: RenderedSnippet) -> Result<RenderedSnippet, HostError> {
    let redacted = redact(&rendered.text);
    if redacted.mostly_secret() {
        return Err(hidden());
    }
    Ok(RenderedSnippet {
        text: redacted.text,
    })
}
