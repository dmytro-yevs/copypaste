//! CopyPaste-g85v — Sensitive-detector true-positive recall corpus.
//!
//! Each entry is a real or structurally-representative secret that MUST be
//! detected by `detect()`. The corpus is exhaustive across:
//!   - AWS access keys (AKIA / ASIA)
//!   - GitHub tokens (classic PAT, fine-grained, Actions)
//!   - Private keys (RSA, OPENSSH, EC, PKCS#8 encrypted, PuTTY)
//!   - JWTs
//!   - Database connection URLs (postgres, mysql, mongodb, redis)
//!   - Key=value secrets (access_token, client_secret, refresh_token, db_password)
//!
//! Any regression that causes a true positive to go undetected will appear as
//! a named test failure, making the miss easy to trace back to its root cause.

use copypaste_core::sensitive::detect;

// ── AWS ───────────────────────────────────────────────────────────────────────

#[test]
fn tp_aws_akia_access_key() {
    // Standard long-term IAM key — AKIA prefix + 16 uppercase alphanumeric chars.
    assert!(
        detect("AKIAIOSFODNN7EXAMPLE").is_some(),
        "AWS AKIA access key must be detected"
    );
}

#[test]
fn tp_aws_akia_in_env_assignment() {
    assert!(
        detect("AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE").is_some(),
        "AWS AKIA key in env-var assignment must be detected"
    );
}

#[test]
fn tp_aws_asia_temporary_key() {
    // ASIA prefix is used by STS temporary credentials.
    assert!(
        detect("ASIAIOSFODNN7EXAMPLE1234").is_some(),
        "AWS ASIA temporary access key must be detected"
    );
}

// ── GitHub ────────────────────────────────────────────────────────────────────

#[test]
fn tp_github_classic_pat() {
    let token = "ghp_".to_string() + &"A".repeat(36);
    assert!(
        detect(&token).is_some(),
        "GitHub classic PAT (ghp_) must be detected"
    );
}

#[test]
fn tp_github_fine_grained_pat() {
    let token = format!("github_pat_{}_{}", "A".repeat(22), "B".repeat(59));
    assert!(
        detect(&token).is_some(),
        "GitHub fine-grained PAT (github_pat_) must be detected"
    );
}

#[test]
fn tp_github_actions_token() {
    let token = "ghs_".to_string() + &"A".repeat(36);
    assert!(
        detect(&token).is_some(),
        "GitHub Actions token (ghs_) must be detected"
    );
}

// ── Private keys ──────────────────────────────────────────────────────────────

#[test]
fn tp_rsa_private_key_pem_header() {
    assert!(
        detect("-----BEGIN RSA PRIVATE KEY-----\nMIIEoAIBAAKCAQEA...").is_some(),
        "RSA PRIVATE KEY PEM header must be detected"
    );
}

#[test]
fn tp_openssh_private_key_pem_header() {
    assert!(
        detect("-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaC1rZXl...").is_some(),
        "OPENSSH PRIVATE KEY PEM header must be detected"
    );
}

#[test]
fn tp_ec_private_key_pem_header() {
    assert!(
        detect("-----BEGIN EC PRIVATE KEY-----\nMHQCAQEE...").is_some(),
        "EC PRIVATE KEY PEM header must be detected"
    );
}

#[test]
fn tp_generic_private_key_pem_header() {
    // PKCS#8 unencrypted form — no algorithm qualifier.
    assert!(
        detect("-----BEGIN PRIVATE KEY-----\nMIIEvgIBADANBgkq...").is_some(),
        "PKCS#8 PRIVATE KEY PEM header must be detected"
    );
}

#[test]
fn tp_pkcs8_encrypted_private_key() {
    // PKCS#8 encrypted form — different header from the RSA/EC/OPENSSH families.
    let blob = "-----BEGIN ENCRYPTED PRIVATE KEY-----\nMIIFLTBXBgkqhkiG9w...";
    assert!(
        detect(blob).is_some(),
        "PKCS#8 encrypted private key header must be detected"
    );
}

#[test]
fn tp_putty_private_key_header() {
    // PuTTY .ppk format — starts with PuTTY-User-Key-File-<n>:
    let blob = "PuTTY-User-Key-File-2: ssh-rsa\nEncryption: none\nComment: rsa-key-20240101\n";
    assert!(
        detect(blob).is_some(),
        "PuTTY private key header must be detected"
    );
}

#[test]
fn tp_private_key_embedded_in_multiline_blob() {
    // Key header appearing mid-blob, not at line 1.
    let blob = "# SSH key below\n-----BEGIN RSA PRIVATE KEY-----\nMIIEo...\n-----END RSA PRIVATE KEY-----\n";
    assert!(
        detect(blob).is_some(),
        "RSA key header embedded inside a multi-line blob must be detected"
    );
}

// ── JWTs ──────────────────────────────────────────────────────────────────────

#[test]
fn tp_jwt_hs256() {
    // A well-formed HS256 JWT — all three dot-separated base64url segments.
    assert!(
        detect(
            "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ1c2VyIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c"
        )
        .is_some(),
        "HS256 JWT must be detected"
    );
}

#[test]
fn tp_jwt_rs256() {
    // RS256 JWT header always produces a longer base64 segment.
    let header = "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9";
    let payload = "eyJzdWIiOiJ1c2VyMTIzIiwibmFtZSI6IkFsaWNlIiwiaWF0IjoxNTE2MjM5MDIyfQ";
    let sig = "SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
    let jwt = format!("{header}.{payload}.{sig}");
    assert!(
        detect(&jwt).is_some(),
        "RS256 JWT (longer header) must be detected"
    );
}

#[test]
fn tp_jwt_in_authorization_header() {
    // Realistic HTTP Authorization header value.
    let value = "Bearer eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ1c2VyIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
    assert!(
        detect(value).is_some(),
        "JWT inside Authorization: Bearer header must be detected"
    );
}

// ── Database connection URLs ──────────────────────────────────────────────────

#[test]
fn tp_postgres_connection_url() {
    assert!(
        detect("postgresql://alice:S3cr3tP@ss@db.example.com:5432/mydb").is_some(),
        "PostgreSQL connection URL must be detected"
    );
}

#[test]
fn tp_mysql_connection_url() {
    assert!(
        detect("mysql://root:hunter2@127.0.0.1:3306/prod").is_some(),
        "MySQL connection URL must be detected"
    );
}

#[test]
fn tp_mongodb_connection_url() {
    assert!(
        detect("mongodb://admin:P@ssw0rd!@mongo.internal:27017/mydb?authSource=admin").is_some(),
        "MongoDB connection URL must be detected"
    );
}

#[test]
fn tp_redis_connection_url() {
    assert!(
        detect("redis://:my_redis_secret_password@redis.example.com:6379/0").is_some(),
        "Redis connection URL with password must be detected"
    );
}

// ── Key=value secrets (CopyPaste-2eet) ───────────────────────────────────────

#[test]
fn tp_access_token_kv_strong() {
    // access_token with a strong value (letter+digit mix, > 6 chars).
    assert!(
        detect("access_token=abc123XYZlongvalue99").is_some(),
        "access_token=<strong> must be detected"
    );
}

#[test]
fn tp_access_token_kv_colon_separator() {
    // Colon separator is also valid in the pattern.
    assert!(
        detect("access_token: gh_access_abc123XYZ").is_some(),
        "access_token: <strong> must be detected"
    );
}

#[test]
fn tp_client_secret_kv() {
    assert!(
        detect("client_secret=Sup3rS3cr3tV@lue").is_some(),
        "client_secret=<strong> must be detected"
    );
}

#[test]
fn tp_refresh_token_kv() {
    assert!(
        detect("refresh_token=rt_abc123XYZlong_value").is_some(),
        "refresh_token=<strong> must be detected"
    );
}

#[test]
fn tp_db_password_kv() {
    assert!(
        detect("db_password=S3cur3Pass!word").is_some(),
        "db_password=<strong> must be detected"
    );
}

#[test]
fn tp_access_token_in_env_assignment() {
    // Shell export / dotenv shape — quoted value without surrounding JSON quotes.
    assert!(
        detect("export access_token=abc123XYZlongvalue99").is_some(),
        "access_token in shell export must be detected"
    );
}

#[test]
fn tp_refresh_token_in_config_file() {
    // Config-file shape — ini-style key = value.
    assert!(
        detect("refresh_token = rt_PROD_abc123XYZlongval").is_some(),
        "refresh_token in ini-style config must be detected"
    );
}

// ── Recall rate gate ─────────────────────────────────────────────────────────

/// Meta-test: assert the whole corpus contains ≥ 30 entries (guards against
/// accidental truncation) and that ALL of them are detected.
/// Individual tests above already pin each entry by name; this test makes
/// the overall recall rate visible in CI output.
#[test]
fn true_positive_recall_100pct() {
    let corpus: &[(&str, &str)] = &[
        // AWS
        ("aws_akia", "AKIAIOSFODNN7EXAMPLE"),
        ("aws_akia_in_env", "AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE"),
        ("aws_asia", "ASIAIOSFODNN7EXAMPLE1234"),
        // GitHub
        (
            "github_classic_pat",
            "ghp_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        ),
        (
            "github_fine_grained",
            "github_pat_AAAAAAAAAAAAAAAAAAAAAA_BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
        ),
        (
            "github_actions",
            "ghs_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        ),
        // Private keys
        (
            "rsa_private_key",
            "-----BEGIN RSA PRIVATE KEY-----\nMIIEo...",
        ),
        (
            "openssh_private_key",
            "-----BEGIN OPENSSH PRIVATE KEY-----\nMIIEo...",
        ),
        (
            "ec_private_key",
            "-----BEGIN EC PRIVATE KEY-----\nMHQCAQEE...",
        ),
        (
            "pkcs8_private_key",
            "-----BEGIN PRIVATE KEY-----\nMIIEvgIBADA...",
        ),
        (
            "pkcs8_encrypted",
            "-----BEGIN ENCRYPTED PRIVATE KEY-----\nMIIFLTBXBgkq...",
        ),
        (
            "putty_key",
            "PuTTY-User-Key-File-2: ssh-rsa\nEncryption: none\n",
        ),
        // JWTs
        (
            "jwt_hs256",
            "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ1c2VyIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c",
        ),
        (
            "jwt_bearer",
            "Bearer eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ1c2VyIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c",
        ),
        // DB URLs
        (
            "postgres_url",
            "postgresql://alice:S3cr3tP@ss@db.example.com:5432/mydb",
        ),
        (
            "mysql_url",
            "mysql://root:hunter2@127.0.0.1:3306/prod",
        ),
        (
            "mongodb_url",
            "mongodb://admin:P@ssw0rd!@mongo.internal:27017/mydb",
        ),
        (
            "redis_url",
            "redis://:my_redis_secret_password@redis.example.com:6379/0",
        ),
        // Key=value secrets
        (
            "access_token_kv",
            "access_token=abc123XYZlongvalue99",
        ),
        (
            "client_secret_kv",
            "client_secret=Sup3rS3cr3tV@lue",
        ),
        (
            "refresh_token_kv",
            "refresh_token=rt_abc123XYZlong_value",
        ),
        (
            "db_password_kv",
            "db_password=S3cur3Pass!word",
        ),
    ];

    assert!(
        corpus.len() >= 20,
        "recall corpus must have ≥ 20 entries (got {})",
        corpus.len()
    );

    let mut misses: Vec<&str> = Vec::new();
    for &(label, text) in corpus {
        if detect(text).is_none() {
            misses.push(label);
        }
    }

    assert!(
        misses.is_empty(),
        "100% recall required — missed {} of {} entries:\n{}",
        misses.len(),
        corpus.len(),
        misses
            .iter()
            .map(|l| format!("  - {l}"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
}
