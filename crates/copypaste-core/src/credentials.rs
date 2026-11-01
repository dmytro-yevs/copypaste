//! Local credential recognition shared by capture, storage and presentation.

use base64::{engine::general_purpose, Engine};
use copypaste_ipc::ClipboardPrivacy;
use zeroize::Zeroizing;

pub(crate) fn metadata(
    privacy: ClipboardPrivacy,
    metadata: Option<&crate::PayloadMetadata>,
) -> crate::PayloadMetadata {
    let mut metadata = metadata.cloned().unwrap_or_default();
    metadata.credentials = true;
    metadata.privacy = metadata.privacy.union(privacy);
    if let Some(choice) = metadata.secret_override {
        metadata.privacy.secret = choice.secret;
    }
    metadata
}

/// Recognize credential syntax, without validating or contacting its issuer.
/// `Some` with empty privacy denotes a public SSH key.
#[must_use]
pub fn classify_credentials(content_type: &str, content: &str) -> Option<ClipboardPrivacy> {
    if !copypaste_ipc::content_type::is_text(content_type) {
        return None;
    }
    let private_key = [
        "-----BEGIN OPENSSH PRIVATE KEY-----",
        "-----BEGIN RSA PRIVATE KEY-----",
        "-----BEGIN DSA PRIVATE KEY-----",
        "-----BEGIN EC PRIVATE KEY-----",
        "-----BEGIN PRIVATE KEY-----",
        "-----BEGIN ENCRYPTED PRIVATE KEY-----",
    ]
    .iter()
    .any(|header| content.contains(header));
    // A private-key header also protects partially copied keys. Public key
    // headers and certificates deliberately do not imply confidentiality.
    if private_key || content.split(token_boundary).any(is_token) {
        return Some(ClipboardPrivacy {
            secret: true,
            transient: false,
        });
    }
    content
        .lines()
        .any(is_public_ssh_key)
        .then_some(ClipboardPrivacy::default())
}

fn token_boundary(character: char) -> bool {
    !character.is_ascii_alphanumeric() && !matches!(character, '_' | '-' | '.')
}

fn is_token(value: &str) -> bool {
    // Bounds cap decoding and JSON work independently of the clipboard cap.
    if !(20..=64 * 1024).contains(&value.len()) {
        return false;
    }
    let prefixes = [
        "ghp_",
        "gho_",
        "ghu_",
        "ghs_",
        "ghr_",
        "github_pat_",
        "glpat-",
        "xoxb-",
        "xoxp-",
        "xoxa-",
        "xoxr-",
        "xoxs-",
        "xapp-",
    ];
    if prefixes.iter().any(|prefix| {
        value.strip_prefix(prefix).is_some_and(|body| {
            body.len() >= 20
                && body
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
        })
    }) {
        return true;
    }
    is_jwt(value) || (value.ends_with('.') && is_jwt(value.trim_end_matches('.')))
}

fn is_jwt(value: &str) -> bool {
    let mut parts = value.split('.');
    let (Some(header), Some(claims), Some(signature)) = (parts.next(), parts.next(), parts.next())
    else {
        return false;
    };
    if parts.next().is_some()
        || header.len() > 4096
        || claims.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        return false;
    }
    let Ok(header) = general_purpose::URL_SAFE_NO_PAD.decode(header) else {
        return false;
    };
    let Ok(header) = serde_json::from_slice::<serde_json::Value>(&header) else {
        return false;
    };
    if !header["alg"].is_string() {
        return false;
    }
    if signature.is_empty() && header["alg"] != "none" {
        return false;
    }
    let Ok(claims) = general_purpose::URL_SAFE_NO_PAD.decode(claims) else {
        return false;
    };
    let claims = Zeroizing::new(claims);
    // IgnoredAny validates JSON without retaining copies of the claim values.
    claims.first() == Some(&b'{')
        && serde_json::from_slice::<serde::de::IgnoredAny>(&claims).is_ok()
        && general_purpose::URL_SAFE_NO_PAD.decode(signature).is_ok()
}

fn is_public_ssh_key(line: &str) -> bool {
    let mut parts = line.split_whitespace();
    let (Some(kind), Some(encoded)) = (parts.next(), parts.next()) else {
        return false;
    };
    if !matches!(kind, "ssh-rsa" | "ssh-dss" | "ssh-ed25519")
        && !kind.starts_with("ecdsa-sha2-")
        && !kind.starts_with("sk-ssh-ed25519@")
        && !kind.starts_with("sk-ecdsa-sha2-")
    {
        return false;
    }
    if encoded.len() > 16 * 1024 {
        return false;
    }
    let Ok(decoded) = general_purpose::STANDARD.decode(encoded) else {
        return false;
    };
    let Some(length) = decoded.get(..4) else {
        return false;
    };
    let length = u32::from_be_bytes(length.try_into().expect("four bytes")) as usize;
    length == kind.len()
        && decoded.get(4..4 + length) == Some(kind.as_bytes())
        && decoded.len() > 4 + length
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_private_keys_and_embedded_provider_tokens() {
        assert!(
            classify_credentials("text", "-----BEGIN OPENSSH PRIVATE KEY-----\npartial")
                .unwrap()
                .secret
        );
        assert!(
            classify_credentials("text", "-----BEGIN ENCRYPTED PRIVATE KEY-----\npartial")
                .unwrap()
                .secret
        );
        for prefix in ["ghp_", "github_pat_", "glpat-", "xoxb-"] {
            let text = format!("TOKEN='{prefix}{}'", "Ab3d".repeat(10));
            assert!(classify_credentials("text", &text).unwrap().secret);
        }
        let jwt = format!(
            "{}.{}.{}",
            general_purpose::URL_SAFE_NO_PAD.encode(br#"{"alg":"HS256"}"#),
            general_purpose::URL_SAFE_NO_PAD.encode(br#"{"sub":"synthetic"}"#),
            general_purpose::URL_SAFE_NO_PAD.encode([1u8; 32])
        );
        assert!(
            classify_credentials("text", &format!("Bearer {jwt}"))
                .unwrap()
                .secret
        );
    }

    #[test]
    fn public_keys_are_credentials_without_becoming_secrets() {
        let mut bytes = Vec::from(11u32.to_be_bytes());
        bytes.extend_from_slice(b"ssh-ed25519");
        bytes.extend_from_slice(&32u32.to_be_bytes());
        bytes.extend_from_slice(&[7u8; 32]);
        let key = format!(
            "ssh-ed25519 {} test@example.org",
            general_purpose::STANDARD.encode(bytes)
        );
        assert_eq!(
            classify_credentials("text", &key),
            Some(ClipboardPrivacy::default())
        );
        assert!(
            classify_credentials("text", &format!("{key}\n-----BEGIN PRIVATE KEY-----"))
                .unwrap()
                .secret
        );
    }

    #[test]
    fn does_not_guess_from_random_strings_prose_or_binary_content() {
        for text in [
            "ssh-ed25519 is a key type",
            "ghp_example",
            "abc.def.ghi",
            "A very long ordinary sentence about API tokens.",
            "-----BEGIN PUBLIC KEY-----",
            "0123456789abcdef0123456789abcdef",
        ] {
            assert_eq!(classify_credentials("text", text), None);
        }
        assert_eq!(
            classify_credentials("image/png", "-----BEGIN PRIVATE KEY-----"),
            None
        );
    }
}
