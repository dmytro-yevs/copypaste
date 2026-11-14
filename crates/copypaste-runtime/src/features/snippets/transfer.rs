//! Snippet bundle, JSON and CSV import and export.
//!
//! A preview parses the file and keeps at most one plan per runtime: a new
//! preview, a commit, `ImportCancel`, `Release` (a surface closed) or a
//! 10-minute deadline releases it, whichever comes first, and its rows are
//! zeroized. `ImportCancel` during a commit stops it before the next row and
//! the transaction writes nothing. Paths and passphrases never reach the
//! log or an error message; logs carry counts only.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use copypaste_core::now_ms;
use copypaste_core::snippets::bundle::TransferError;
use copypaste_core::snippets::{export, import};
use copypaste_core::storage::snippets::SnippetError;
use copypaste_ipc::snippets::{ImportPlan, SnippetOp};
use copypaste_ipc::{ErrorCode, LibraryKinds, Response, ResponseData};

use crate::deadlines::DeadlineKey;
use crate::messages::storage_error;
use crate::Runtime;

/// How long an unused plan is kept.
const PLAN_TTL_MS: i64 = 10 * 60 * 1000;
const DEADLINE_OWNER: &str = "snippet_import";

const MSG_PATH: &str = "Choose a file by its full location.";
const MSG_NO_FILE: &str = "That file or folder does not exist.";
const MSG_FILE: &str = "The file could not be read or written.";
const MSG_PASSPHRASE_REQUIRED: &str = "This file is encrypted. Enter its passphrase.";
const MSG_WRONG_PASSPHRASE: &str = "The passphrase does not open this file.";
const MSG_WEAK_PASSPHRASE: &str = "A passphrase needs at least 8 characters.";
const MSG_DAMAGED: &str = "The encrypted file is damaged.";
const MSG_MALFORMED: &str = "The file is not in the chosen format.";
const MSG_NEWER: &str = "The file was made by a newer version of CopyPaste.";
const MSG_COLUMNS: &str = "The CSV file needs a body column.";
const MSG_TOO_LARGE: &str = "The file is larger than 50 MB.";
const MSG_TOO_MANY: &str = "The file has more than 10,000 snippets.";
const MSG_PLAN_GONE: &str = "This import preview has ended. Preview the file again.";
const MSG_LIMIT: &str = "The snippet library is full.";

/// Plans held in this process, at most one per runtime.
static PLANS: Mutex<Vec<Outstanding>> = Mutex::new(Vec::new());

struct Outstanding {
    runtime: usize,
    token: String,
    /// `None` while the plan is being committed.
    plan: Option<import::ImportPlan>,
    stop: Arc<AtomicBool>,
}

fn plans() -> MutexGuard<'static, Vec<Outstanding>> {
    PLANS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn runtime_key(runtime: &Runtime) -> usize {
    std::ptr::from_ref(runtime).addr()
}

fn deadline(token: &str) -> DeadlineKey {
    DeadlineKey::new(DEADLINE_OWNER, token)
}

/// Drops this runtime's plan, if any is waiting; a commit in progress
/// carries on.
pub(super) fn release(runtime: &Runtime) {
    let key = runtime_key(runtime);
    let mut released = Vec::new();
    plans().retain_mut(|outstanding| {
        let waiting = outstanding.runtime == key && outstanding.plan.is_some();
        if waiting {
            released.push(outstanding.token.clone());
        }
        !waiting
    });
    for token in released {
        runtime.deadlines.cancel(&deadline(&token));
    }
}

/// Drops the plan named `token`; a commit using it is told to stop.
fn cancel(runtime: &Runtime, token: &str) -> bool {
    let key = runtime_key(runtime);
    let mut found = false;
    plans().retain_mut(|outstanding| {
        if outstanding.runtime != key || outstanding.token != token {
            return true;
        }
        found = true;
        outstanding.stop.store(true, Ordering::Relaxed);
        outstanding.plan.is_none()
    });
    runtime.deadlines.cancel(&deadline(token));
    found
}

pub(super) fn handle(runtime: &Runtime, id: u64, op: SnippetOp) -> Response {
    match op {
        SnippetOp::ImportPreview {
            path,
            passphrase,
            format,
            convert_placeholders,
        } => {
            release(runtime);
            let Some(path) = absolute(&path) else {
                return Response::err(id, ErrorCode::InvalidRequest, MSG_PATH);
            };
            let parsed = import::read(path, passphrase.as_deref(), format, convert_placeholders)
                .and_then(|parsed| import::plan(&runtime.store, parsed));
            match parsed {
                Ok(plan) => Response::ok(id, ResponseData::SnippetImportPlan(keep(runtime, plan))),
                Err(error) => refusal(id, "snippet_import_preview", &error),
            }
        }
        SnippetOp::ImportCommit { token, policies } => {
            let key = runtime_key(runtime);
            let taken = plans()
                .iter_mut()
                .find(|outstanding| outstanding.runtime == key && outstanding.token == token)
                .and_then(|outstanding| {
                    let plan = outstanding.plan.take()?;
                    Some((plan, Arc::clone(&outstanding.stop)))
                });
            let Some((plan, stop)) = taken else {
                return Response::err(id, ErrorCode::NotFound, MSG_PLAN_GONE);
            };
            runtime.deadlines.cancel(&deadline(&token));
            let started = now_ms();
            let committed = import::commit(
                &runtime.store,
                &runtime.keyring.item_key(),
                plan,
                policies,
                &|| stop.load(Ordering::Relaxed),
            );
            plans()
                .retain(|outstanding| !(outstanding.runtime == key && outstanding.token == token));
            match committed {
                Ok(report) => {
                    if report.inserted + report.updated > 0 {
                        runtime.library_written(
                            started,
                            LibraryKinds::SNIPPETS.union(LibraryKinds::COLLECTIONS),
                        );
                    }
                    tracing::debug!(
                        inserted = report.inserted,
                        updated = report.updated,
                        skipped = report.skipped,
                        "snippet import committed"
                    );
                    Response::ok(id, ResponseData::SnippetImportReport(report))
                }
                Err(error) => refusal(id, "snippet_import_commit", &error),
            }
        }
        SnippetOp::ImportCancel { token } => {
            cancel(runtime, &token);
            Response::ok(id, ResponseData::Empty {})
        }
        SnippetOp::Export {
            path,
            scope,
            passphrase,
            include_concealed,
        } => {
            let Some(path) = absolute(&path) else {
                return Response::err(id, ErrorCode::InvalidRequest, MSG_PATH);
            };
            match export::export(
                &runtime.store,
                path,
                &scope,
                passphrase.as_deref(),
                include_concealed,
            ) {
                Ok(written) => {
                    tracing::debug!(written, "snippets exported");
                    Response::ok(id, ResponseData::Count(u64::from(written)))
                }
                Err(error) => refusal(id, "snippet_export", &error),
            }
        }
        _ => crate::features::unsupported(id),
    }
}

fn absolute(path: &str) -> Option<&Path> {
    Some(Path::new(path)).filter(|path| path.is_absolute())
}

/// Holds `plan` as this runtime's one outstanding plan until its deadline.
fn keep(runtime: &Runtime, plan: import::ImportPlan) -> ImportPlan {
    let token = plan.token().to_owned();
    let wire = ImportPlan {
        token: token.clone(),
        counts: plan.counts(),
        rows: plan.rows(),
    };
    tracing::debug!(rows = plan_rows(&wire), "snippet import planned");
    let key = runtime_key(runtime);
    let mut held = plans();
    held.retain(|outstanding| outstanding.runtime != key || outstanding.plan.is_none());
    held.push(Outstanding {
        runtime: key,
        token: token.clone(),
        plan: Some(plan),
        stop: Arc::new(AtomicBool::new(false)),
    });
    drop(held);
    let expiring = token.clone();
    runtime.deadlines.schedule(
        deadline(&token),
        runtime.deadlines.now().saturating_add(PLAN_TTL_MS),
        Box::new(move |runtime| {
            let key = runtime_key(runtime);
            plans().retain(|outstanding| {
                outstanding.runtime != key
                    || outstanding.token != expiring
                    || outstanding.plan.is_none()
            });
        }),
    );
    wire
}

fn plan_rows(plan: &ImportPlan) -> u32 {
    let counts = plan.counts;
    counts.new + counts.identical + counts.updated + counts.abbreviation_conflicts + counts.invalid
}

fn refusal(id: u64, operation: &'static str, error: &TransferError) -> Response {
    let (code, message) = match error {
        TransferError::Io(error) if error.kind() == std::io::ErrorKind::NotFound => {
            (ErrorCode::NotFound, MSG_NO_FILE)
        }
        TransferError::Io(error) => {
            tracing::warn!(operation, kind = ?error.kind(), "snippet file failed");
            (ErrorCode::Internal, MSG_FILE)
        }
        TransferError::PassphraseRequired => (ErrorCode::AuthFailed, MSG_PASSPHRASE_REQUIRED),
        TransferError::WrongPassphrase => (ErrorCode::AuthFailed, MSG_WRONG_PASSPHRASE),
        TransferError::WeakPassphrase => (ErrorCode::InvalidRequest, MSG_WEAK_PASSPHRASE),
        TransferError::Damaged => (ErrorCode::InvalidRequest, MSG_DAMAGED),
        TransferError::Malformed => (ErrorCode::InvalidRequest, MSG_MALFORMED),
        TransferError::UnsupportedVersion => (ErrorCode::InvalidRequest, MSG_NEWER),
        TransferError::MissingColumns => (ErrorCode::InvalidRequest, MSG_COLUMNS),
        TransferError::TooLarge => (ErrorCode::ContentTooLarge, MSG_TOO_LARGE),
        TransferError::TooManyRows => (ErrorCode::ContentTooLarge, MSG_TOO_MANY),
        TransferError::Cancelled => (ErrorCode::NotFound, MSG_PLAN_GONE),
        TransferError::Snippet(SnippetError::LimitReached) => {
            (ErrorCode::InvalidRequest, MSG_LIMIT)
        }
        TransferError::Snippet(SnippetError::Store(error)) => {
            return storage_error(id, operation, error)
        }
        TransferError::Snippet(_) => {
            tracing::warn!(operation, "snippet import was refused by the store");
            (ErrorCode::Internal, MSG_FILE)
        }
    };
    Response::err(id, code, message)
}

#[cfg(test)]
mod tests {
    use copypaste_ipc::snippets::{ExportScope, ImportFormat, ImportPolicies};
    use copypaste_ipc::Method;

    use super::*;
    use crate::tests::fixture;

    fn request(runtime: &Runtime, op: SnippetOp) -> Response {
        runtime.request_blocking(1, Method::Snippets { op })
    }

    fn preview(runtime: &Runtime, path: &Path) -> String {
        let op = SnippetOp::ImportPreview {
            path: path.to_string_lossy().into_owned(),
            passphrase: None,
            format: ImportFormat::Json,
            convert_placeholders: false,
        };
        match request(runtime, op).data {
            Some(ResponseData::SnippetImportPlan(plan)) => plan.token,
            other => panic!("unexpected {other:?}"),
        }
    }

    fn commit(runtime: &Runtime, token: &str) -> Response {
        request(
            runtime,
            SnippetOp::ImportCommit {
                token: token.to_owned(),
                policies: ImportPolicies::default(),
            },
        )
    }

    #[test]
    fn one_plan_is_outstanding_until_a_new_preview_release_or_its_deadline() {
        let (runtime, dir) = fixture();
        let file = dir.path().join("snippets.json");
        std::fs::write(&file, r#"[{"name":"Hi","text":"hello","keyword":"hi"}]"#).unwrap();

        let first = preview(&runtime, &file);
        let second = preview(&runtime, &file);
        let replaced = commit(&runtime, &first);
        assert_eq!(replaced.error_code, Some(ErrorCode::NotFound));

        assert!(request(&runtime, SnippetOp::Release).error_code.is_none());
        let released = commit(&runtime, &second);
        assert_eq!(released.error_code, Some(ErrorCode::NotFound));

        let third = preview(&runtime, &file);
        runtime.deadlines.advance(PLAN_TTL_MS + 1);
        runtime.sweep_deadlines();
        let expired = commit(&runtime, &third);
        assert_eq!(expired.error_code, Some(ErrorCode::NotFound));
        let key = runtime_key(&runtime);
        assert!(plans().iter().all(|held| held.runtime != key));

        let fourth = preview(&runtime, &file);
        let Some(ResponseData::SnippetImportReport(report)) = commit(&runtime, &fourth).data else {
            panic!("a report expected");
        };
        assert_eq!(report.inserted, 1);
        assert_eq!(runtime.store.snippet_count().unwrap(), 1);
        let used = commit(&runtime, &fourth);
        assert_eq!(used.error_code, Some(ErrorCode::NotFound));
    }

    #[test]
    fn a_wrong_passphrase_names_no_path_and_a_relative_path_is_refused() {
        let (runtime, dir) = fixture();
        let file = dir.path().join("library.cpsnippets");
        let op = SnippetOp::Export {
            path: file.to_string_lossy().into_owned(),
            scope: ExportScope::All,
            passphrase: Some("long enough".into()),
            include_concealed: false,
        };
        let exported = request(&runtime, op);
        assert!(matches!(exported.data, Some(ResponseData::Count(0))));

        let op = SnippetOp::ImportPreview {
            path: file.to_string_lossy().into_owned(),
            passphrase: Some("not the one".into()),
            format: ImportFormat::Bundle,
            convert_placeholders: false,
        };
        let refused = request(&runtime, op);
        assert_eq!(refused.error_code, Some(ErrorCode::AuthFailed));
        let message = refused.error.unwrap_or_default();
        assert!(!message.contains('/') && !message.contains('\\'));

        let op = SnippetOp::ImportPreview {
            path: "snippets.json".into(),
            passphrase: None,
            format: ImportFormat::Json,
            convert_placeholders: false,
        };
        let relative = request(&runtime, op);
        assert_eq!(relative.error_code, Some(ErrorCode::InvalidRequest));
    }
}
