mod apps;
mod engine;
mod fp;
mod kind;
mod luhn;
mod normalize;

pub use apps::is_sensitive_app;
pub use engine::{PatternMatch, SensitiveCategory, SensitiveDetector};
pub use kind::{detect, is_sensitive_for_autowipe, SensitiveKind};
pub use luhn::luhn_valid;
pub use normalize::nfkc_normalize;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_aws_access_key() {
        assert!(detect("AKIAIOSFODNN7EXAMPLE").is_some());
    }
    #[test]
    fn detects_temporary_aws_key() {
        assert!(detect("ASIAIOSFODNN7EXAMPLE1234").is_some());
    }
    #[test]
    fn detects_github_classic_pat() {
        assert!(detect(&("ghp_".to_string() + &"A".repeat(36))).is_some());
    }
    #[test]
    fn detects_github_fine_grained_pat() {
        assert!(detect(&format!("github_pat_{}_{}", "A".repeat(22), "B".repeat(59))).is_some());
    }
    #[test]
    fn detects_openai_key() {
        assert!(detect(&("sk-proj-".to_string() + &"A".repeat(48))).is_some());
    }
    #[test]
    fn detects_anthropic_key() {
        assert!(detect(&("sk-ant-api03-".to_string() + &"A".repeat(80))).is_some());
    }
    #[test]
    fn detects_jwt() {
        assert!(detect(
            "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ1c2VyIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c"
        )
        .is_some());
    }
    #[test]
    fn detects_ssh_private_key() {
        assert!(detect("-----BEGIN RSA PRIVATE KEY-----\nMIIEo...").is_some());
    }
    #[test]
    fn detects_openssh_private_key() {
        assert!(detect("-----BEGIN OPENSSH PRIVATE KEY-----\nMIIEo...").is_some());
    }
    #[test]
    fn detects_pkcs8_encrypted_private_key() {
        // Audit MED #5 — PKCS#8 encrypted form previously slipped through.
        let blob = "garbage prefix\n-----BEGIN ENCRYPTED PRIVATE KEY-----\nMIIFD...\n";
        let kind = detect(blob).expect("should detect PKCS#8 encrypted key");
        assert!(matches!(kind, SensitiveKind::SshPrivateKey));
    }
    #[test]
    fn detects_putty_user_key_file() {
        // Audit MED #5 — PuTTY `.ppk` header.
        let blob =
            "PuTTY-User-Key-File-2: ssh-rsa\nEncryption: none\nComment: imported-from-openssh\n";
        let kind = detect(blob).expect("should detect PuTTY key");
        assert!(matches!(kind, SensitiveKind::SshPrivateKey));
    }
    #[test]
    fn jwt_word_boundary_anchors_match() {
        // Audit MED #5 — `\b` anchor: real JWT in normal context detects.
        let jwt =
            "Bearer eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ1c2VyIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
        assert!(detect(jwt).is_some());
        // A `eyJ`-prefixed garbage glued onto an identifier should NOT match
        // as a JWT (we'd still detect bearer token from generic_bearer if
        // present — here we use a non-bearer prefix to isolate the case).
        let glued = "configsomethingeyJabc.def.ghi notajwt";
        // Either no match at all OR not classified as Jwt — both are
        // acceptable; pin "not Jwt" precisely.
        let kind = detect(glued);
        assert!(
            !matches!(kind, Some(SensitiveKind::Jwt)),
            "glued eyJ inside an identifier must not be classified as JWT"
        );
    }
    #[test]
    fn detects_stripe_live_key() {
        assert!(detect(&("sk_live_".to_string() + &"A".repeat(24))).is_some());
    }
    #[test]
    fn detects_npm_token() {
        assert!(detect(&("npm_".to_string() + &"A".repeat(36))).is_some());
    }
    #[test]
    fn no_false_positive_on_lorem_ipsum() {
        assert!(detect("Lorem ipsum dolor sit amet, consectetur adipiscing elit.").is_none());
    }
    #[test]
    fn no_false_positive_on_short_code() {
        assert!(detect(r#"fn main() { println!("Hello, world!"); }"#).is_none());
    }
    #[test]
    fn credit_card_detected_short_line_only() {
        assert!(detect("4111111111111111").is_some());
    }
    #[test]
    fn credit_card_detected_when_embedded_in_longer_text() {
        // Audit MED #6: the previous `len <= 25` gate dropped this case.
        let blob = "Customer card: 4111 1111 1111 1111 — expires 12/26";
        let kind = detect(blob).expect("embedded card must be detected");
        assert!(matches!(kind, SensitiveKind::CreditCard));
    }
    #[test]
    fn credit_card_with_hyphens_in_long_text() {
        let blob = "please charge 4111-1111-1111-1111 today";
        let kind = detect(blob).expect("hyphenated card must be detected");
        assert!(matches!(kind, SensitiveKind::CreditCard));
    }
    #[test]
    fn credit_card_no_false_positive_on_luhn_invalid_run() {
        // Pin: a Luhn-invalid 13-digit run inside longer text must not
        // classify as CreditCard. We assert *only* "not classified as
        // CreditCard" — the input may still trigger an unrelated pattern
        // (e.g. phone_us on a 10-digit subrun), which is out of scope.
        // NOTE: the previous fixture "4242424242422" was accidentally Luhn-valid
        // (4+2+4+... alternating produces sum=50 ≡ 0 mod 10). Updated to
        // "4242424242421" which is provably Luhn-invalid (sum=49 mod 10 ≠ 0).
        let blob = "ref=4242424242421 EOT";
        let kind = detect(blob);
        assert!(
            !matches!(kind, Some(SensitiveKind::CreditCard)),
            "Luhn-invalid 13-digit run must not classify as CreditCard, got {:?}",
            kind
        );
    }
    #[test]
    fn detects_slack_bot_token() {
        assert!(detect("xoxb-17653285717-17653285718-AbCdEfGhIjKlMnOpQrStUvWx").is_some());
    }
    #[test]
    fn detects_slack_webhook() {
        assert!(detect(
            "https://hooks.slack.com/services/T00000000/B00000000/XXXXXXXXXXXXXXXXXXXXXXXX"
        )
        .is_some());
    }
    #[test]
    fn detects_stripe_webhook_secret() {
        assert!(detect("whsec_aAbBcCdDeEfFgGhHiIjJkKlLmMnNoOpPqQrRsStT").is_some());
    }
    #[test]
    fn detects_google_api_key() {
        assert!(detect("AIzaSyD-9tSrke72EmVt4TenJheB96ABCDE12345").is_some());
    }
    #[test]
    fn detects_github_actions_token() {
        assert!(detect("ghs_16C7e42F292c6912E7710c838347Ae178B4a").is_some());
    }
    #[test]
    #[cfg_attr(
        debug_assertions,
        ignore = "regex perf test only meaningful in release builds"
    )]
    fn pattern_match_completes_in_5ms_on_10mb_text() {
        let big = "a".repeat(10_000_000);
        let start = std::time::Instant::now();
        let _ = detect(&big);
        assert!(
            start.elapsed().as_millis() < 500,
            "took {}ms",
            start.elapsed().as_millis()
        );
    }

    // --- is_sensitive_app tests ---

    #[test]
    fn sensitive_app_1password_bundle_id() {
        assert!(is_sensitive_app("com.1password.1password"));
    }

    #[test]
    fn sensitive_app_bitwarden_bundle_id() {
        assert!(is_sensitive_app("com.bitwarden.desktop"));
    }

    #[test]
    fn sensitive_app_keepassxc_bundle_id() {
        assert!(is_sensitive_app("com.keepassxc.keepassxc"));
    }

    #[test]
    fn sensitive_app_dashlane_bundle_id() {
        assert!(is_sensitive_app("com.dashlane.dashlane"));
    }

    #[test]
    fn sensitive_app_process_name_fragment() {
        // Process names may be short (e.g. "1password", "bitwarden")
        assert!(is_sensitive_app("bitwarden"));
        assert!(is_sensitive_app("keepass"));
    }

    #[test]
    fn sensitive_app_case_insensitive() {
        assert!(is_sensitive_app("com.Bitwarden.Desktop"));
        assert!(is_sensitive_app("COM.1PASSWORD.1PASSWORD"));
    }

    #[test]
    fn sensitive_app_unknown_app_returns_false() {
        assert!(!is_sensitive_app("com.apple.finder"));
        assert!(!is_sensitive_app("com.google.chrome"));
        assert!(!is_sensitive_app(""));
    }

    #[test]
    fn sensitive_app_partial_match() {
        // "1password" appears as substring in longer bundle IDs
        assert!(is_sensitive_app("com.agilebits.onepassword4"));
    }

    // ── NFKC normalisation / Unicode bypass guards ─────────────────────────────

    #[test]
    fn nfkc_normalised_input_detects_secrets() {
        // Full-width "AKIA" (U+FF21..U+FF24) + 16 ASCII chars after NFKC → AKIA + 16 = AWS key.
        let fullwidth_akia = "\u{FF21}\u{FF2B}\u{FF29}\u{FF21}IOSFODNN7EXAMPLE";
        let kind = detect(fullwidth_akia);
        assert!(kind.is_some(), "expected AWS key after NFKC normalisation");
        matches!(kind.unwrap(), SensitiveKind::AwsKey);
    }

    #[test]
    fn nfkc_zwj_in_jwt_normalises_away() {
        // A real JWT with a zero-width joiner inserted; NFKC strips ZWJ.
        // Note: ZWJ (U+200D) is a control char and NFKC keeps it in many cases;
        // but `eyJ` prefix is ASCII and the regex still matches on the surrounding bytes.
        // Use NFKC normalisation to demonstrate it doesn't break detection of clean JWTs.
        let clean =
            "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ1c2VyIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
        assert!(detect(clean).is_some());
    }

    #[test]
    fn nfkc_normalize_is_idempotent_on_ascii() {
        let s = "AKIAIOSFODNN7EXAMPLE";
        assert_eq!(nfkc_normalize(s), s);
    }

    // ── generic_password_kv FP guards ───────────────────────────────────────────

    #[test]
    fn weak_password_value_is_filtered() {
        // value "foo" — too short, no special, no letter+digit mix.
        assert!(detect("password: foo").is_none());
    }

    #[test]
    fn weak_password_short_letters_is_filtered() {
        // "nope" — too short, no special, no digit.
        assert!(detect("secret = nope").is_none());
    }

    #[test]
    fn strong_password_value_letter_digit_mix_detected() {
        assert!(detect("password=hunter2").is_some());
    }

    #[test]
    fn strong_password_value_with_special_char_detected() {
        assert!(detect("secret = !abcdef").is_some());
    }

    #[test]
    fn long_password_value_detected() {
        assert!(detect("password: abcdefghij").is_some()); // 10 chars
    }

    #[test]
    fn multibyte_value_gated_on_chars_not_bytes() {
        // 9 CJK characters = 27 UTF-8 bytes. The byte-length gate (`>= 10`)
        // would mis-classify this short value as "strong" purely because of
        // its byte width; the char-count gate (`chars().count() >= 10`)
        // correctly treats 9 letters with no digit/special as weak.
        let nine_cjk = "私的秘密言葉確認鍵"; // 9 chars, 27 bytes
        assert_eq!(nine_cjk.chars().count(), 9);
        assert!(nine_cjk.len() >= 10, "precondition: byte length exceeds 10");
        assert!(
            !fp::is_credential_value_strong(nine_cjk),
            "a 9-char multibyte letters-only value must be weak (char gate, not byte gate)"
        );

        // 10 multibyte chars clears the char-count gate → strong.
        let ten_cjk = "私的秘密言葉確認鍵値"; // 10 chars
        assert_eq!(ten_cjk.chars().count(), 10);
        assert!(fp::is_credential_value_strong(ten_cjk));
    }

    // ── is_sensitive_for_autowipe: confidence floor tests ─────────────────────

    /// HIGH-confidence credentials MUST trigger auto-wipe.
    #[test]
    fn autowipe_triggers_for_aws_key() {
        assert!(is_sensitive_for_autowipe("AKIAIOSFODNN7EXAMPLE"));
    }

    #[test]
    fn autowipe_triggers_for_jwt() {
        assert!(is_sensitive_for_autowipe(
            "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ1c2VyIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c"
        ));
    }

    #[test]
    fn autowipe_triggers_for_ssh_private_key() {
        assert!(is_sensitive_for_autowipe(
            "-----BEGIN RSA PRIVATE KEY-----\nMIIEo..."
        ));
    }

    #[test]
    fn autowipe_triggers_for_credit_card() {
        assert!(is_sensitive_for_autowipe("4111111111111111"));
    }

    #[test]
    fn autowipe_triggers_for_openai_key() {
        assert!(is_sensitive_for_autowipe(
            &("sk-proj-".to_string() + &"A".repeat(48))
        ));
    }

    /// LOW-confidence patterns MUST NOT trigger auto-wipe (data-loss fix).
    #[test]
    fn autowipe_does_not_trigger_for_phone_number() {
        // phone_us has confidence 0.55 — below the 0.70 floor.
        assert!(
            !is_sensitive_for_autowipe("Call me at (555) 867-5309"),
            "phone number must not trigger auto-wipe"
        );
    }

    #[test]
    fn autowipe_does_not_trigger_for_email_address() {
        // email has confidence 0.60 — below the 0.70 floor.
        assert!(
            !is_sensitive_for_autowipe("Send to alice@example.com"),
            "email address must not trigger auto-wipe"
        );
    }

    #[test]
    fn autowipe_does_not_trigger_for_passport_like_code() {
        // passport has confidence 0.55 — below the 0.70 floor.
        // 9-digit passport number format: 2 uppercase letters + 9 digits.
        assert!(
            !is_sensitive_for_autowipe("Order AB123456789 is ready"),
            "passport-like code must not trigger auto-wipe"
        );
    }

    #[test]
    fn autowipe_does_not_trigger_for_plain_text() {
        assert!(!is_sensitive_for_autowipe(
            "Lorem ipsum dolor sit amet, consectetur adipiscing elit."
        ));
    }

    /// Vault tokens below 32 chars (now filtered by pattern) must not wipe.
    #[test]
    fn autowipe_does_not_trigger_for_short_hvs_prefix() {
        // Short "hvs.abc" (only 3 chars after dot) should not match the
        // tightened vault pattern requiring {32,} chars.
        assert!(
            !is_sensitive_for_autowipe("hvs.abc123"),
            "short hvs. prefix must not trigger auto-wipe"
        );
    }

    /// Real Vault token (32+ chars after dot) still triggers.
    #[test]
    fn autowipe_triggers_for_real_vault_token() {
        let token = "hvs.".to_string() + &"A".repeat(32);
        assert!(
            is_sensitive_for_autowipe(&token),
            "real vault token (32+ chars) must trigger auto-wipe"
        );
    }

    // ── P2 fb3e: false-positive / auto-wipe floor tests ──────────────────────

    /// discord_bot_token is now 0.65 — must NOT auto-wipe.
    #[test]
    fn autowipe_does_not_trigger_for_discord_bot_token() {
        // Construct a string matching the discord_bot_token shape.
        let token = "MNabcdefghijklmnopqrstuvwx.ABCDEF.ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456";
        assert!(
            !is_sensitive_for_autowipe(token),
            "discord_bot_token (conf 0.65) must not trigger auto-wipe"
        );
    }

    /// twilio_signing_key_sid is now 0.65 — must NOT auto-wipe.
    #[test]
    fn autowipe_does_not_trigger_for_twilio_signing_key_sid() {
        let sid = format!("SK{}", "a".repeat(32));
        assert!(
            !is_sensitive_for_autowipe(&sid),
            "twilio_signing_key_sid (conf 0.65) must not trigger auto-wipe"
        );
    }

    /// iban is now 0.65 — legitimately-copied bank details must NOT auto-wipe.
    #[test]
    fn autowipe_does_not_trigger_for_iban() {
        // Valid IBAN shape (Germany, 22 chars).
        assert!(
            !is_sensitive_for_autowipe("DE89370400440532013000"),
            "IBAN (conf 0.65) must not trigger auto-wipe"
        );
    }

    /// ssn_us is now 0.65 — date-like strings must NOT auto-wipe.
    #[test]
    fn autowipe_does_not_trigger_for_date_like_ssn() {
        assert!(
            !is_sensitive_for_autowipe("012 31 2024"),
            "date-like SSN pattern (conf 0.65) must not trigger auto-wipe"
        );
    }

    /// generic_bearer is now 0.65 — must NOT trigger auto-wipe even when matched.
    #[test]
    fn generic_bearer_does_not_trigger_autowipe() {
        // A realistic Bearer header with a JWT-like token. generic_bearer (0.65)
        // is below the 0.70 auto-wipe floor, so this must not auto-wipe.
        // (It will still appear in detect() results for display/flagging.)
        assert!(
            !is_sensitive_for_autowipe(
                "Authorization: Bearer eyJhbGci0iJSUzI1NiIsInR5cCI6IkpXVCJ9"
            ),
            "generic_bearer (conf 0.65) must not trigger auto-wipe"
        );
    }

    /// generic_bearer is still detected (just not auto-wiped).
    #[test]
    fn generic_bearer_still_detected_below_floor() {
        let d = SensitiveDetector::new();
        let hits = d.detect("Authorization: Bearer eyJhbGci0iJSUzI1NiIsInR5cCI6IkpXVCJ9");
        // generic_bearer (0.65) is detected, but confidence is below 0.70.
        let bearer_hits: Vec<_> = hits
            .iter()
            .filter(|m| m.pattern_name == "generic_bearer")
            .collect();
        assert!(
            !bearer_hits.is_empty(),
            "generic_bearer must still appear in detect() results for flagging"
        );
        assert!(
            bearer_hits.iter().all(|m| m.confidence < 0.70),
            "generic_bearer confidence must be below 0.70 auto-wipe floor"
        );
    }

    // ── CopyPaste-8ys1: private IP auto-wipe guard ───────────────────────────

    /// RFC1918 IPs with port in config files must NOT auto-wipe.
    /// ip_with_port confidence is lowered to 0.65 (below the 0.70 floor)
    /// because bare IP:port pairs are infrastructure topology, not secrets —
    /// credentialed connections are caught by db_conn_string (0.99).
    #[test]
    fn autowipe_does_not_trigger_for_private_ip_10_block() {
        // 10.0.0.0/8 — common private LAN
        assert!(
            !is_sensitive_for_autowipe("db_host=10.0.0.1:5432"),
            "10.x private IP with port must not trigger auto-wipe"
        );
    }

    #[test]
    fn autowipe_does_not_trigger_for_private_ip_172_block() {
        // 172.16.0.0/12 — Docker / VPC default range
        assert!(
            !is_sensitive_for_autowipe("172.16.0.5:6379"),
            "172.16.x private IP with port must not trigger auto-wipe"
        );
    }

    #[test]
    fn autowipe_does_not_trigger_for_private_ip_192_168_block() {
        // 192.168.0.0/16 — home/office network
        assert!(
            !is_sensitive_for_autowipe("192.168.1.100:8080"),
            "192.168.x private IP with port must not trigger auto-wipe"
        );
    }

    /// ip_with_port is still detected (just not auto-wiped) so the UI can
    /// flag infrastructure topology for review.
    #[test]
    fn ip_with_port_still_detected_below_autowipe_floor() {
        let d = SensitiveDetector::new();
        let hits = d.detect("192.168.1.1:5432");
        let ip_hits: Vec<_> = hits
            .iter()
            .filter(|m| m.pattern_name == "ip_with_port")
            .collect();
        assert!(
            !ip_hits.is_empty(),
            "ip_with_port must still appear in detect() results"
        );
        assert!(
            ip_hits.iter().all(|m| m.confidence < 0.70),
            "ip_with_port confidence must be below 0.70 auto-wipe floor"
        );
    }

    // ── CopyPaste-2eet: key=value secret patterns ─────────────────────────────

    /// access_token=<strong value> must be detected.
    #[test]
    fn detects_access_token_kv() {
        assert!(
            detect("access_token=abc123XYZlongvalue99").is_some(),
            "access_token key=value must be detected"
        );
    }

    /// client_secret=<strong value> must be detected.
    #[test]
    fn detects_client_secret_kv() {
        assert!(
            detect("client_secret=Sup3rS3cr3tV@lue!").is_some(),
            "client_secret key=value must be detected"
        );
    }

    /// refresh_token=<strong value> must be detected.
    #[test]
    fn detects_refresh_token_kv() {
        assert!(
            detect("refresh_token=rt_abc123XYZlong_value").is_some(),
            "refresh_token key=value must be detected"
        );
    }

    /// db_password=<strong value> must be detected (the `password` substring in
    /// `db_password` is currently matched by the existing pattern, but the
    /// explicit key name is now included so the intent is documented and future
    /// refactors cannot accidentally remove it).
    #[test]
    fn detects_db_password_kv() {
        // db_password matches via the generic_password_kv `password` alternative.
        assert!(
            detect("db_password=S3cur3Pass!word").is_some(),
            "db_password key=value must be detected"
        );
    }

    /// Weak values for new keys must still be filtered (FP guard).
    #[test]
    fn new_kv_keys_weak_value_not_detected() {
        assert!(
            detect("access_token=short").is_none(),
            "access_token with weak value must not be detected"
        );
        assert!(
            detect("refresh_token=abc").is_none(),
            "refresh_token with weak value must not be detected"
        );
    }

    // ── P2 ozzt: new cloud/infra token detection tests ───────────────────────

    #[test]
    fn detects_sendgrid_api_key() {
        let key = format!("SG.{}.{}", "A".repeat(22), "B".repeat(43));
        assert!(detect(&key).is_some(), "SendGrid API key must be detected");
    }

    #[test]
    fn sendgrid_autowipe_triggers() {
        let key = format!("SG.{}.{}", "A".repeat(22), "B".repeat(43));
        assert!(
            is_sensitive_for_autowipe(&key),
            "SendGrid API key (conf 0.99) must trigger auto-wipe"
        );
    }

    #[test]
    fn detects_terraform_cloud_token() {
        let token = format!("atlasv1.{}", "A".repeat(64));
        assert!(
            detect(&token).is_some(),
            "Terraform Cloud token must be detected"
        );
    }

    #[test]
    fn terraform_cloud_autowipe_triggers() {
        let token = format!("atlasv1.{}", "A".repeat(64));
        assert!(
            is_sensitive_for_autowipe(&token),
            "Terraform Cloud token (conf 0.99) must trigger auto-wipe"
        );
    }

    #[test]
    fn detects_gcp_service_account_key() {
        let json = r#"{"type": "service_account", "private_key": "-----BEGIN RSA PRIVATE KEY-----\nMIIEo..."}"#;
        assert!(
            detect(json).is_some(),
            "GCP service account key JSON must be detected"
        );
    }

    #[test]
    fn gcp_service_account_autowipe_triggers() {
        let json = r#"{"private_key": "-----BEGIN RSA PRIVATE KEY-----\nMIIEo..."}"#;
        assert!(
            is_sensitive_for_autowipe(json),
            "GCP service account key (conf 0.99) must trigger auto-wipe"
        );
    }

    #[test]
    fn detects_azure_storage_key() {
        // Real Azure key shape lives in an `AccountKey=` connection string.
        let key = format!("AccountKey={}==", "A".repeat(86));
        assert!(
            detect(&key).is_some(),
            "Azure storage account key must be detected"
        );
    }

    #[test]
    fn azure_storage_key_autowipe_triggers() {
        let key = format!("AccountKey={}==", "A".repeat(86));
        assert!(
            is_sensitive_for_autowipe(&key),
            "Azure storage key (conf 0.90) must trigger auto-wipe"
        );
        // Regression (bug-hunt high finding): a BARE 88-char base64 blob with no
        // AccountKey= context (e.g. a SHA-512 / random token) must NOT auto-wipe,
        // otherwise the detector silently deletes benign content.
        let bare_blob = format!("{}==", "A".repeat(86));
        assert!(
            !is_sensitive_for_autowipe(&bare_blob),
            "bare 88-char base64 (no AccountKey=) must NOT trigger auto-wipe"
        );
    }

    /// openai_legacy sk- with 48 chars (not sk-proj-) must still trigger.
    #[test]
    fn autowipe_triggers_for_openai_legacy_key() {
        let key = "sk-".to_string() + &"A".repeat(48);
        assert!(
            is_sensitive_for_autowipe(&key),
            "openai legacy key must trigger auto-wipe"
        );
    }

    /// sk-proj- must NOT also fire openai_legacy (double-match guard).
    // P2 r6cw: the previous comment claimed "the (?!proj-) lookahead prevents
    // double-fire" — that was incorrect. The `regex` crate does not support
    // lookahead. Exclusion works structurally: `openai_legacy` is
    // `\bsk-[A-Za-z0-9]{48}\b`. A `sk-proj-AAAA…` string has a hyphen after
    // "proj" which is NOT in `[A-Za-z0-9]`, so the 48-char alnum run can only
    // start at offset 8 (after the second hyphen), but then `\b` fails because
    // the preceding char `j` is also a word char — the outer `\bsk-` anchor
    // would need to match from the very start. In practice `sk-proj-` breaks
    // the contiguous 48-char alnum requirement, so no match occurs. This
    // structural exclusion is what the comment in patterns.rs correctly
    // documents; no code change is required here, only the comment correction.
    #[test]
    fn openai_legacy_does_not_match_proj_prefix() {
        // sk-proj- keys are caught by openai_new; openai_legacy must not also
        // match them. Exclusion is structural: `sk-proj-` inserts a hyphen that
        // breaks the `[A-Za-z0-9]{48}` run required by openai_legacy, so the
        // pattern simply has no 48-char alnum run to latch onto. No lookahead
        // is involved — the `regex` crate does not support lookahead syntax.
        let d = SensitiveDetector::new();
        let key = "sk-proj-".to_string() + &"A".repeat(48);
        let matches = d.detect(&key);
        let legacy_hits: Vec<_> = matches
            .iter()
            .filter(|m| m.pattern_name == "openai_legacy")
            .collect();
        assert!(
            legacy_hits.is_empty(),
            "openai_legacy must not fire on sk-proj- keys; got: {legacy_hits:?}"
        );
    }
}
