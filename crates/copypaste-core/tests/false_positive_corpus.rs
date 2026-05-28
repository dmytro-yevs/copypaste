//! Wave 2.8 — Sensitive-detector false-positive corpus.
//!
//! Sweeps a curated set of benign messages through `detect()` and asserts the
//! false-positive rate stays at or below 5% (i.e. ≤ 2 of 50 entries flagged).
//!
//! This guards against regex over-eagerness, especially the `generic_password_kv`
//! pattern that previously matched everyday prose containing the word "password".

use copypaste_core::sensitive::detect;

/// Benign clipboard content that MUST NOT be classified as sensitive.
/// Sources span casual prose, forum posts, source-code comments, configuration
/// docs, support-ticket excerpts, and ordinary technical writing.
const BENIGN_CORPUS: &[&str] = &[
    // Forum / prose
    "the password is great, you should try it",
    "my secret is to drink coffee every morning",
    "I forgot my password again, time to reset it",
    "the secret of life is to enjoy the small things",
    "password protected zip files are common",
    "what's the secret? hard work and patience",
    "remember to set a strong password for your account",
    "the auth token expired, please log in again",
    "Don't share your password with anyone",
    "she told me her secret was a healthy diet",
    // Comments / docs
    "// example: set api_key=demo to enable test mode",
    "# password: <set in your env file>",
    "/* secret = TBD, fill in before deploy */",
    "// TODO: rotate api_key once a quarter",
    "// the password field accepts up to 64 chars",
    "Note: passwd:enabled means SSH password auth is on",
    "Set apikey: yourkey in the config (do not commit)",
    "// auth_token: see README for setup",
    "// secret: see vault for the real value",
    "# api_key=demo for examples only",
    // Code-ish but benign
    "fn check_password(pw: &str) -> bool { pw.len() > 8 }",
    "if (password.length < 8) { showError(); }",
    "const SECRET_NAME = \"prod-key\";",
    "let api_key = getEnv(); // value loaded later",
    "const password = prompt('enter password:');",
    // Documentation
    "AWS region us-east-1 is recommended",
    "see arn naming conventions in the AWS docs",
    "the GitHub repo URL is https://github.com/example/repo",
    "Slack notifications can be set up via incoming webhooks",
    "GCP IAM roles control access to your project",
    // URLs / paths (no creds)
    "https://example.com/login?next=/dashboard",
    "/var/log/system.log is rotated daily",
    "see https://docs.example.com for setup",
    "the file path is /etc/hosts on most Linux systems",
    "open the file at C:\\Users\\Public\\Documents",
    // Numbers / IDs (not credentials)
    "the order number is 1234567890",
    "tracking ID 0010 0020 0030",
    "ticket #4815 has been assigned to you",
    "version 1.2.3 was released yesterday",
    "build number 9876 passed all checks",
    // Support tickets
    "Customer reports: my password doesn't work after reset",
    "Please reset the password for user alice (no value given)",
    "Issue: secret expired, requesting renewal",
    "The api_key returns 401, please investigate",
    "User cannot log in with their password",
    // Long prose
    "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua",
    "The quick brown fox jumps over the lazy dog, and the dog doesn't seem to mind very much at all",
    "Once upon a time, in a land far away, there lived a wizard who guarded a great secret deep in the woods",
    "Software engineering is largely about managing complexity through abstraction and decomposition",
    "Continuous integration helps teams catch problems early in the development cycle",
];

#[test]
fn false_positive_rate_below_5pct_on_benign_corpus() {
    // Sanity: corpus is at least 50 entries so 5% == 2.5 → cap at 2.
    assert!(
        BENIGN_CORPUS.len() >= 50,
        "corpus must have ≥50 entries (currently {})",
        BENIGN_CORPUS.len()
    );

    let mut false_positives: Vec<(&str, String)> = Vec::new();
    for &message in BENIGN_CORPUS {
        if let Some(kind) = detect(message) {
            false_positives.push((message, format!("{:?}", kind)));
        }
    }

    let allowed = BENIGN_CORPUS.len() * 5 / 100; // 5% of corpus size
    let allowed = allowed.max(2); // never tighter than 2 absolute hits
    assert!(
        false_positives.len() <= allowed,
        "FP rate exceeded 5%: {} of {} benign messages flagged.\nMatches:\n{}",
        false_positives.len(),
        BENIGN_CORPUS.len(),
        false_positives
            .iter()
            .map(|(msg, kind)| format!("  - [{}] {}", kind, msg))
            .collect::<Vec<_>>()
            .join("\n"),
    );
}
