use std::time::Duration;

// ── CloudError ────────────────────────────────────────────────────────────────

/// Errors returned by cloud-sync initialisation.
///
/// All variants are **fail-closed**: callers should treat any error as "do not
/// start cloud sync" rather than "fall back to a less-secure mode".
#[derive(Debug, thiserror::Error)]
pub enum CloudError {
    /// Email/password auth was configured (both `SUPABASE_EMAIL` and
    /// `SUPABASE_PASSWORD` set) but sign-in failed. We refuse to silently
    /// fall back to the anon key — that would downgrade auth scope.
    #[error("Supabase email/password sign-in failed: {0}; refusing to fall back to anon key")]
    AuthFailed(String),

    /// `SUPABASE_URL` did not start with `https://`. Cloud sync over plain
    /// HTTP would leak the anon key and clipboard contents on the wire.
    #[error("Supabase URL must use HTTPS, got: {0}")]
    InsecureUrl(String),

    /// Keychain access failed after the configured retry budget. Daemon
    /// should continue in degraded mode (no cloud sync) and surface this
    /// to the user.
    #[error("Keychain unavailable after retries: {0}; entering degraded mode")]
    KeychainDegraded(String),

    /// The local database file already exists, has the SQLite/SQLCipher magic
    /// header, and we were asked to use an ephemeral encryption key. That
    /// would brick access to the existing data — refuse.
    #[error("Existing encrypted database at {0} cannot be opened with an ephemeral key")]
    EncryptedDbRequiresPersistentKey(String),
}

// ── CloudConfig ───────────────────────────────────────────────────────────────

/// Runtime configuration read from environment variables.
#[derive(Debug, Clone)]
pub struct CloudConfig {
    /// Supabase project base URL, e.g. `https://abc.supabase.co`.
    pub supabase_url: String,
    /// Supabase anonymous/public API key.
    pub anon_key: String,
    /// GoTrue account email for the `authenticated`-scope password grant.
    /// `None` falls back to anon-key-only operation (which the project's
    /// RLS policies reject — see [`resolve_bearer`]).
    pub email: Option<String>,
    /// GoTrue account password. Never logged.
    pub password: Option<String>,
}

impl CloudConfig {
    /// Returns `Some(config)` if both `SUPABASE_URL` and `SUPABASE_ANON_KEY`
    /// are available, checking (in order):
    /// 1. `SUPABASE_URL` / `SUPABASE_ANON_KEY` environment variables.
    /// 2. The persisted [`crate::ipc::AppConfig`] (`supabase_url` /
    ///    `supabase_anon_key` fields set via the UI's `set_config` IPC call).
    ///
    /// This lets the UI configure Supabase credentials without requiring the
    /// operator to set environment variables manually.
    ///
    /// **Email/password resolution** (for the `authenticated`-scope GoTrue
    /// sign-in) mirrors the URL/key resolution: `SUPABASE_EMAIL` /
    /// `SUPABASE_PASSWORD` env vars take precedence, then the persisted
    /// `AppConfig` (`supabase_email` / `supabase_password`, written by
    /// `copypaste cloud setup` into the same `0600` `config.json`). Persisting
    /// them is required so the documented one-command setup yields a daemon
    /// that authenticates — anon-key-only requests are rejected by the
    /// `authenticated`-only RLS policies and sync silently fails otherwise.
    ///
    /// **Scheme validation** happens at `start_cloud` time via
    /// [`CloudError::InsecureUrl`], not here.
    pub fn from_env() -> Option<Self> {
        let app_cfg = crate::ipc::read_config();

        // Email/password: env var wins, else persisted config. Empty values are
        // treated as absent so a blank env export doesn't shadow stored creds.
        let nonempty = |s: String| if s.trim().is_empty() { None } else { Some(s) };
        let email = std::env::var("SUPABASE_EMAIL")
            .ok()
            .and_then(nonempty)
            .or_else(|| app_cfg.supabase_email.clone().and_then(nonempty));
        // Password resolution (item 1): env var → Keychain → config.json fallback
        // (migration: old installs that still have the password in config.json are
        // served until the next set_config call migrates it to the Keychain).
        let password = std::env::var("SUPABASE_PASSWORD")
            .ok()
            .and_then(nonempty)
            .or_else(|| crate::keychain::read_supabase_password_from_keychain().and_then(nonempty))
            .or_else(|| app_cfg.supabase_password.clone().and_then(nonempty));

        // Priority 1: environment variables for URL + anon key.
        if let (Ok(url), Ok(key)) = (
            std::env::var("SUPABASE_URL"),
            std::env::var("SUPABASE_ANON_KEY"),
        ) {
            return Some(Self {
                supabase_url: url.trim_end_matches('/').to_owned(),
                anon_key: key,
                email,
                password,
            });
        }
        // Priority 2: persisted AppConfig (set via the UI or `cloud setup`).
        let url = app_cfg.supabase_url?;
        let key = app_cfg.supabase_anon_key?;
        Some(Self {
            supabase_url: url.trim_end_matches('/').to_owned(),
            anon_key: key,
            email,
            password,
        })
    }

    /// Construct + validate a [`CloudConfig`]. Rejects non-HTTPS URLs eagerly.
    /// Prefer this in tests and any new call sites; `from_env` is preserved
    /// for backward compatibility with the existing daemon wiring.
    pub fn new(supabase_url: String, anon_key: String) -> Result<Self, CloudError> {
        let trimmed = supabase_url.trim_end_matches('/').to_owned();
        if !is_https_url(&trimmed) {
            return Err(CloudError::InsecureUrl(supabase_url));
        }
        Ok(Self {
            supabase_url: trimmed,
            anon_key,
            email: None,
            password: None,
        })
    }
}

/// Strict HTTPS check. We deliberately do **not** pull in the `url` crate for
/// this — a string-prefix check plus a sanity test that something follows the
/// scheme is sufficient, and avoids a transitive-dep surface.
///
/// Accepts: `https://host[:port][/path...]`
/// Rejects: `http://...`, `ws://...`, `file://...`, bare hostnames, empty strings.
pub(crate) fn is_https_url(s: &str) -> bool {
    // Use a case-insensitive scheme compare; reject if no authority follows.
    let lower = s.to_ascii_lowercase();
    if !lower.starts_with("https://") {
        return false;
    }
    let rest = &s[8..];
    // Must have at least one non-`/` character (a host).
    rest.chars()
        .next()
        .is_some_and(|c| c != '/' && !c.is_whitespace())
}

/// TEST-ONLY HTTPS-gate relaxation.
///
/// Returns `true` only when the URL is plain `http://` pointing at a loopback
/// host (`127.0.0.1`/`localhost`/`[::1]`). This lets the test suite point the
/// cloud orchestrator at an in-process mock PostgREST bound to loopback.
///
/// In production this function does not exist: the `#[cfg(not(test))]` variant
/// is a hard `false`, so [`start_cloud`] always demands HTTPS in the shipped
/// binary. Loopback HTTP is never trusted outside the test harness.
#[cfg(test)]
pub(super) fn test_only_allows_local_http(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    let Some(rest) = lower.strip_prefix("http://") else {
        return false;
    };
    // Host is everything up to the first `/`, `:` (port), or end-of-string.
    let host = rest.split(['/', ':']).next().unwrap_or_default();
    matches!(host, "127.0.0.1" | "localhost" | "[::1]" | "::1")
}

/// Production stub: loopback HTTP is NEVER allowed. Always `false` so the HTTPS
/// gate in [`start_cloud`] is absolute in the shipped binary.
#[cfg(not(test))]
#[inline]
pub(super) fn test_only_allows_local_http(_s: &str) -> bool {
    false
}

/// Redact an account email for logging / error payloads. The account email is
/// PII and must never appear verbatim in logs or surfaced errors. We keep just
/// enough structure to be useful for debugging — the first character of the
/// local part and the domain — and mask the rest:
///
/// - `alice@example.com` → `a***@example.com`
/// - `a@example.com`     → `*@example.com`
/// - `not-an-email`      → `<redacted>`
pub(crate) fn redact_email(email: &str) -> String {
    match email.split_once('@') {
        Some((local, domain)) if !local.is_empty() && !domain.is_empty() => {
            let first = local.chars().next().unwrap_or('*');
            if local.chars().count() <= 1 {
                format!("*@{domain}")
            } else {
                format!("{first}***@{domain}")
            }
        }
        // No `@` (or empty local/domain): not a recognisable address — never
        // echo it back, since it may still be sensitive operator input.
        _ => "<redacted>".to_string(),
    }
}

// ── Pre-flight checks ─────────────────────────────────────────────────────────

/// Probe `crate::keychain::load_or_create` with a one-shot retry policy
/// (3 attempts, exponential backoff: 100ms, 300ms, 900ms).
///
/// Returns `Ok(())` on success or [`CloudError::KeychainDegraded`] after
/// exhausting retries. Crucially, this is *bounded* — we never loop forever
/// even if the keychain entry has been deleted or the user denies access.
#[cfg(target_os = "macos")]
pub async fn probe_keychain_with_retry() -> Result<(), CloudError> {
    probe_with_retry(|| match crate::keychain::load_or_create() {
        Ok(_) => Ok(()),
        Err(e) => Err(e.to_string()),
    })
    .await
}

/// Non-macOS stub: there is no keychain to probe; always returns `Ok(())`
/// (the caller is already using an ephemeral key by design on these platforms).
#[cfg(not(target_os = "macos"))]
pub async fn probe_keychain_with_retry() -> Result<(), CloudError> {
    Ok(())
}

/// Generic bounded-retry probe: 3 attempts with exponential backoff
/// (100ms, 300ms between retries). Injected as a closure so we can write
/// deterministic tests without touching the real keychain (which would
/// block on interactive prompts in dev environments).
pub(crate) async fn probe_with_retry<F>(mut probe: F) -> Result<(), CloudError>
where
    F: FnMut() -> Result<(), String>,
{
    let mut last_err = String::new();
    let mut delay_ms = 100u64;
    for attempt in 1..=3 {
        match probe() {
            Ok(()) => return Ok(()),
            Err(e) => {
                last_err = e;
                tracing::warn!("keychain probe attempt {attempt}/3 failed: {last_err}");
            }
        }
        if attempt < 3 {
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            delay_ms *= 3;
        }
    }
    Err(CloudError::KeychainDegraded(last_err))
}

/// SQLite / SQLCipher file-header magic. SQLite databases (encrypted or not)
/// begin with the ASCII string `SQLite format 3\0` (16 bytes). SQLCipher v4
/// uses the same prefix because the first 16 bytes are reserved for this
/// magic by the SQLite file format; an actively-encrypted SQLCipher DB will
/// instead start with the *encrypted* version of that header (random-looking
/// bytes), so the safest check is: "file exists AND is at least 16 bytes
/// long AND is non-empty".
pub(crate) const SQLITE_MAGIC: &[u8; 16] = b"SQLite format 3\0";

/// Inspect a putative database file and decide whether it is safe to open with
/// an *ephemeral* encryption key.
///
/// Returns:
/// - `Ok(())` — file does not exist, is empty, or is zero-length. Ephemeral
///   key is safe (fresh DB or no DB at all).
/// - `Err(CloudError::EncryptedDbRequiresPersistentKey)` — file exists with
///   ≥16 bytes of content. We cannot tell whether it is a plain SQLite DB
///   with the magic header or a SQLCipher DB with random-looking ciphertext,
///   but in either case a freshly-generated ephemeral key will not decrypt
///   it, so refuse rather than corrupt user data.
pub fn preflight_encrypted_db_check(db_path: &std::path::Path) -> Result<(), CloudError> {
    use std::io::Read;
    let mut f = match std::fs::File::open(db_path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        // Permission / IO error: be conservative and refuse to silently use
        // an ephemeral key for a file we cannot inspect.
        Err(e) => {
            return Err(CloudError::EncryptedDbRequiresPersistentKey(format!(
                "{}: cannot inspect ({e})",
                db_path.display()
            )));
        }
    };
    let mut buf = [0u8; 16];
    match f.read(&mut buf) {
        Ok(0) => Ok(()),           // empty file, treat as fresh
        Ok(n) if n < 16 => Ok(()), // partial write or truncated — still safe-ish (not a real DB)
        Ok(_) => {
            // Either plain SQLite ("SQLite format 3\0") or SQLCipher (encrypted
            // header). Either way, an ephemeral key is wrong.
            let is_plain_sqlite = buf == *SQLITE_MAGIC;
            tracing::error!(
                "refusing ephemeral key: existing DB at {} (plain_sqlite={})",
                db_path.display(),
                is_plain_sqlite
            );
            Err(CloudError::EncryptedDbRequiresPersistentKey(
                db_path.display().to_string(),
            ))
        }
        Err(e) => Err(CloudError::EncryptedDbRequiresPersistentKey(format!(
            "{}: read error ({e})",
            db_path.display()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    // ── HTTPS validation ──────────────────────────────────────────────────────

    #[test]
    fn cloud_rejects_non_https_supabase_url() {
        // http:// is rejected
        let err = CloudConfig::new("http://abc.supabase.co".to_owned(), "anon".to_owned())
            .expect_err("plain http must be rejected");
        match err {
            CloudError::InsecureUrl(u) => assert_eq!(u, "http://abc.supabase.co"),
            other => panic!("expected InsecureUrl, got {other:?}"),
        }

        // other schemes are rejected
        for url in ["ws://abc.supabase.co", "file:///etc/passwd", "ftp://x", ""] {
            assert!(
                CloudConfig::new(url.to_owned(), "anon".to_owned()).is_err(),
                "url {url:?} should be rejected"
            );
        }

        // https:// is accepted (with and without trailing slash)
        let cfg = CloudConfig::new("https://abc.supabase.co/".to_owned(), "anon".to_owned())
            .expect("https url must be accepted");
        assert_eq!(cfg.supabase_url, "https://abc.supabase.co");

        // case-insensitive scheme is also accepted
        assert!(
            CloudConfig::new("HTTPS://abc.supabase.co".to_owned(), "anon".to_owned()).is_ok(),
            "uppercase HTTPS scheme should be accepted"
        );
    }

    #[test]
    fn redact_email_masks_pii() {
        assert_eq!(redact_email("alice@example.com"), "a***@example.com");
        assert_eq!(redact_email("a@example.com"), "*@example.com");
        // No usable @ → fully redacted, never echoed.
        assert_eq!(redact_email("not-an-email"), "<redacted>");
        assert_eq!(redact_email("@example.com"), "<redacted>");
        assert_eq!(redact_email("user@"), "<redacted>");
        assert_eq!(redact_email(""), "<redacted>");
        // The full local part beyond the first char must never survive.
        let r = redact_email("dmitriy.evseev.99@gmail.com");
        assert!(!r.contains("evseev"), "local part leaked: {r}");
        assert_eq!(r, "d***@gmail.com");
    }

    #[test]
    fn is_https_url_helper_edge_cases() {
        assert!(is_https_url("https://x.test"));
        assert!(is_https_url("https://x.test:8443/api"));
        assert!(!is_https_url("https://"));
        assert!(!is_https_url("https:///"));
        assert!(!is_https_url("http://x.test"));
        assert!(!is_https_url("not-a-url"));
    }

    // ── Keychain degraded mode ────────────────────────────────────────────────

    /// The retry helper must:
    ///   1. Stop after exactly 3 attempts (no crash loop).
    ///   2. Surface `CloudError::KeychainDegraded` carrying the last error.
    ///   3. Complete inside the backoff budget (≈0.4s = 100ms + 300ms).
    ///
    /// We inject a closure that always errors so the test is deterministic
    /// and does NOT touch the real macOS keychain (which would block on
    /// interactive prompts in dev environments — the very failure mode this
    /// helper is designed to bound).
    #[tokio::test(flavor = "current_thread")]
    async fn keychain_missing_enters_degraded_mode_no_crash_loop() {
        let attempts = std::cell::Cell::new(0u32);
        let probe = || -> Result<(), String> {
            attempts.set(attempts.get() + 1);
            Err(format!("simulated keychain miss #{}", attempts.get()))
        };

        let start = std::time::Instant::now();
        let result = tokio::time::timeout(Duration::from_secs(2), probe_with_retry(probe))
            .await
            .expect("probe must complete inside 2s — proves no crash loop");
        let elapsed = start.elapsed();

        // Exactly 3 attempts — bounded retry budget.
        assert_eq!(attempts.get(), 3, "must attempt exactly 3 times, no more");

        // Total elapsed: 100ms + 300ms backoff ≈ 400ms (allow generous slack).
        assert!(
            elapsed < Duration::from_secs(1),
            "probe budget exceeded: {elapsed:?}; degraded mode must be reached promptly"
        );

        // Must surface CloudError::KeychainDegraded with the last attempt's message.
        match result {
            Err(CloudError::KeychainDegraded(msg)) => {
                assert!(msg.contains("simulated keychain miss #3"), "got: {msg}");
            }
            other => panic!("expected KeychainDegraded after 3 failures, got {other:?}"),
        }
    }

    /// Symmetric: if the very first probe succeeds, no retries happen and
    /// the helper returns `Ok(())` immediately.
    #[tokio::test(flavor = "current_thread")]
    async fn keychain_probe_succeeds_first_attempt_no_retry() {
        let attempts = std::cell::Cell::new(0u32);
        let probe = || -> Result<(), String> {
            attempts.set(attempts.get() + 1);
            Ok(())
        };
        probe_with_retry(probe)
            .await
            .expect("first-attempt success");
        assert_eq!(attempts.get(), 1, "must not retry after success");
    }

    // ── Encrypted-DB preflight ────────────────────────────────────────────────

    #[test]
    fn preflight_allows_missing_db() {
        let path = std::path::PathBuf::from("/tmp/copypaste-test-does-not-exist-xyz123.db");
        let _ = std::fs::remove_file(&path);
        assert!(preflight_encrypted_db_check(&path).is_ok());
    }

    #[test]
    fn preflight_allows_empty_db_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("empty.db");
        std::fs::File::create(&path).unwrap();
        assert!(preflight_encrypted_db_check(&path).is_ok());
    }

    #[test]
    fn preflight_rejects_existing_sqlite_db() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("real.db");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(SQLITE_MAGIC).unwrap();
        f.write_all(&[0u8; 100]).unwrap();
        let err = preflight_encrypted_db_check(&path)
            .expect_err("existing SQLite DB must block ephemeral-key path");
        assert!(matches!(
            err,
            CloudError::EncryptedDbRequiresPersistentKey(_)
        ));
    }

    #[test]
    fn preflight_rejects_sqlcipher_encrypted_db() {
        // SQLCipher-encrypted DB: first 16 bytes are random-looking ciphertext,
        // NOT the plain SQLite magic. We still refuse — we cannot decrypt
        // without a persistent key.
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("cipher.db");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(&[0xDEu8; 16]).unwrap();
        f.write_all(&[0xADu8; 200]).unwrap();
        let err = preflight_encrypted_db_check(&path)
            .expect_err("existing encrypted DB must also block ephemeral key");
        assert!(matches!(
            err,
            CloudError::EncryptedDbRequiresPersistentKey(_)
        ));
    }
}
