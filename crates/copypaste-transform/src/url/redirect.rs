//! Offline unwrapping of known redirect wrappers: search-engine result
//! links, social outbound-link wrappers and mail link scanners. The target
//! is read from the wrapper itself; nothing is ever fetched, so shorteners
//! stay as they are.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;

/// Wrappers nested deeper than this are left partly wrapped.
const MAX_LAYERS: usize = 4;

/// The innermost http(s) target of `url`, if it is a known wrapper.
pub(crate) fn unwrap(url: &str) -> Option<String> {
    let mut current: Option<String> = None;
    for _ in 0..MAX_LAYERS {
        match target(current.as_deref().unwrap_or(url)) {
            Some(next) => current = Some(next),
            None => break,
        }
    }
    current
}

fn target(url: &str) -> Option<String> {
    let parsed = ::url::Url::parse(url).ok()?;
    let host = parsed.host_str()?.to_ascii_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host);
    let path = parsed.path();
    let param = |name: &str| {
        parsed
            .query_pairs()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.into_owned())
    };
    let target = match wrapper(host, path)? {
        Wrapper::Param("q") => param("q").or_else(|| param("url"))?,
        Wrapper::Param(name) => param(name)?,
        Wrapper::Bing => bing_target(&param("u")?)?,
    };
    is_http(&target).then_some(target)
}

enum Wrapper {
    /// The target is this query parameter.
    Param(&'static str),
    Bing,
}

/// Which wrapper `host` and `path` (host without `www.`) are, if any.
fn wrapper(host: &str, path: &str) -> Option<Wrapper> {
    let facebook = matches!(
        host,
        "l.facebook.com" | "lm.facebook.com" | "l.messenger.com"
    );
    let wrapper = match path {
        "/url" if is_google(host) => Wrapper::Param("q"),
        "/ck/a" if host == "bing.com" => Wrapper::Bing,
        "/l/" if host.ends_with("duckduckgo.com") => Wrapper::Param("uddg"),
        "/l.php" if facebook => Wrapper::Param("u"),
        "/link" if host == "slack-redir.net" => Wrapper::Param("url"),
        "/redir/redirect" | "/safety/go" if host == "linkedin.com" => Wrapper::Param("url"),
        _ if host.ends_with(".safelinks.protection.outlook.com") => Wrapper::Param("url"),
        _ => return None,
    };
    Some(wrapper)
}

/// `google.com`, `google.co.uk`, `google.de` and so on.
fn is_google(host: &str) -> bool {
    host.strip_prefix("google.")
        .is_some_and(|tld| !tld.is_empty() && tld.split('.').count() <= 2)
}

/// Bing writes the target as `a1` followed by unpadded URL-safe Base64.
fn bing_target(value: &str) -> Option<String> {
    let encoded = value.strip_prefix("a1")?;
    let bytes = URL_SAFE_NO_PAD.decode(encoded.trim_end_matches('=')).ok()?;
    String::from_utf8(bytes).ok()
}

fn is_http(target: &str) -> bool {
    let lower = target.get(..8).unwrap_or(target).to_ascii_lowercase();
    (lower.starts_with("http://") || lower.starts_with("https://"))
        && !target.chars().any(|c| c.is_whitespace() || c.is_control())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_wrappers_unwrap_and_unsafe_targets_do_not() {
        let inner = "https://www.google.com/url?q=https%3A%2F%2Fa.test%2Fp%3Fx%3D1&sa=D";
        let outer = format!(
            "https://eur01.safelinks.protection.outlook.com/?url={}&data=1",
            percent_encoding::utf8_percent_encode(inner, percent_encoding::NON_ALPHANUMERIC)
        );
        assert_eq!(unwrap(&outer).as_deref(), Some("https://a.test/p?x=1"));
        assert_eq!(
            unwrap("https://www.google.com/url?q=javascript:alert(1)"),
            None
        );
        assert_eq!(unwrap("https://bit.ly/abc"), None);
    }
}
