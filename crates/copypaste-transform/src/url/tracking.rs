//! Tracking-parameter removal over every http(s) URL in a text.
//!
//! The built-in list holds only global names confirmed by public browser
//! documentation (Firefox query stripping, Brave query filter) plus the
//! `utm_` campaign prefix. Domain-scoped names are not built in; users add
//! them, and allow-list names or domains, through [`TrackingRules`]. Kept
//! parameters keep their order and spelling byte for byte, and links that
//! carry a signature are never touched.

use std::borrow::Cow;

use percent_encoding::percent_decode_str;

use crate::catalogue::{Context, Output};
use crate::error::{Location, TransformError, Warning};
use crate::url::scan::rewrite_urls;

/// Changes whenever the built-in list below changes.
pub const BUILTIN_LIST_VERSION: u32 = 1;

/// Removed from every URL. Names are compared ignoring ASCII case.
const BUILTIN_NAMES: [&str; 10] = [
    "__s",
    "_hsenc",
    "fbclid",
    "gclid",
    "mc_eid",
    "mkt_tok",
    "msclkid",
    "oly_anon_id",
    "oly_enc_id",
    "vero_id",
];

const BUILTIN_PREFIXES: [&str; 1] = ["utm_"];

/// Kept when the link mentions this word, so unsubscribe links still work.
const MARKETING_TOKEN: &str = "mkt_tok";

/// A link carrying one of these is signed: removing anything breaks it.
const SIGNATURE_NAMES: [&str; 4] = ["x-amz-signature", "x-goog-signature", "signature", "sig"];

/// User additions to the built-in list and exceptions to both. Entries are
/// `name`, `name@domain`, `prefix*` or `*@domain`; a domain also matches its
/// subdomains. Malformed entries are ignored.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrackingRules {
    remove: Vec<Rule>,
    allow: Vec<Rule>,
}

impl TrackingRules {
    pub fn new<S: AsRef<str>>(remove: &[S], allow: &[S]) -> Self {
        let parse = |entries: &[S]| {
            entries
                .iter()
                .filter_map(|e| Rule::parse(e.as_ref()))
                .collect()
        };
        Self {
            remove: parse(remove),
            allow: parse(allow),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Rule {
    /// Lowercase name, or prefix when `prefix` is set (empty matches all).
    name: String,
    prefix: bool,
    /// Lowercase ASCII (punycode) host.
    domain: Option<String>,
}

impl Rule {
    fn parse(entry: &str) -> Option<Self> {
        let entry = entry.trim();
        let (name, domain) = match entry.rsplit_once('@') {
            Some((name, domain)) => (name, Some(normalize_domain(domain)?)),
            None => (entry, None),
        };
        let (name, prefix) = match name.strip_suffix('*') {
            Some(stem) => (stem, true),
            None => (name, false),
        };
        if (name.is_empty() && !prefix) || name.contains(['&', '=', '#', '?', '*']) {
            return None;
        }
        Some(Self {
            name: name.to_ascii_lowercase(),
            prefix,
            domain,
        })
    }

    fn matches(&self, name: &str, host: &str) -> bool {
        let name_matches = if self.prefix {
            name.starts_with(&self.name)
        } else {
            name == self.name
        };
        name_matches && self.domain.as_deref().is_none_or(|d| on_domain(host, d))
    }
}

fn normalize_domain(domain: &str) -> Option<String> {
    let domain = domain
        .trim()
        .trim_start_matches("*.")
        .trim_start_matches('.');
    match ::url::Host::parse(domain).ok()? {
        ::url::Host::Domain(ascii) => Some(ascii),
        _ => None,
    }
}

fn on_domain(host: &str, domain: &str) -> bool {
    host.strip_suffix(domain)
        .is_some_and(|rest| rest.is_empty() || rest.ends_with('.'))
}

/// Removes tracking parameters from every http(s) URL in `text`.
pub fn clean_urls(
    text: &str,
    rules: &TrackingRules,
    ctx: &Context,
) -> Result<Output, TransformError> {
    let mut warnings = Vec::new();
    let cleaned = rewrite_urls(text, ctx, |url, offset| {
        let result = clean_url(url, rules);
        if result == Cleaned::Signed {
            warnings.push(Warning::at(
                "url.signed_unchanged",
                Location::of(text, offset),
            ));
        }
        match result {
            Cleaned::Changed(new) => Some(new),
            Cleaned::Unchanged | Cleaned::Signed => None,
        }
    })?;
    Ok(Output {
        text: cleaned,
        warnings,
        sensitive: false,
    })
}

#[derive(Debug, PartialEq, Eq)]
enum Cleaned {
    Changed(String),
    Unchanged,
    Signed,
}

fn clean_url(url: &str, rules: &TrackingRules) -> Cleaned {
    let (base, fragment) = match url.split_once('#') {
        Some((base, fragment)) => (base, Some(fragment)),
        None => (url, None),
    };
    let (path, query) = match base.split_once('?') {
        Some((path, query)) => (path, Some(query)),
        None => (base, None),
    };
    if query.is_none() && fragment.is_none_or(|f| !f.contains('=')) {
        return Cleaned::Unchanged;
    }
    let Some(host) = ::url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_ascii_lowercase))
    else {
        return Cleaned::Unchanged;
    };
    if query.is_some_and(is_signed) {
        return Cleaned::Signed;
    }
    let filter = Filter {
        rules,
        host: &host,
        keep_marketing_token: url.to_ascii_lowercase().contains("unsubscribe"),
    };
    let new_query = query.map(|q| filter.pairs(q));
    let new_fragment = fragment.map(|f| filter.fragment(f));
    let changed =
        matches!(new_query, Some(Cow::Owned(_))) || matches!(new_fragment, Some(Cow::Owned(_)));
    if !changed {
        return Cleaned::Unchanged;
    }
    let mut out = String::with_capacity(url.len());
    out.push_str(path);
    if let Some(query) = new_query.filter(|q| !q.is_empty()) {
        out.push('?');
        out.push_str(&query);
    }
    if let Some(fragment) = new_fragment.filter(|f| !f.is_empty()) {
        out.push('#');
        out.push_str(&fragment);
    }
    Cleaned::Changed(out)
}

/// The decoded, lowercased name of one `name=value` pair.
fn pair_name(pair: &str) -> String {
    let raw = pair.split_once('=').map_or(pair, |(name, _)| name);
    let spaced = raw.replace('+', " ");
    percent_decode_str(&spaced)
        .decode_utf8_lossy()
        .to_ascii_lowercase()
}

fn is_signed(query: &str) -> bool {
    query
        .split('&')
        .any(|pair| SIGNATURE_NAMES.contains(&pair_name(pair).as_str()))
}

struct Filter<'a> {
    rules: &'a TrackingRules,
    host: &'a str,
    keep_marketing_token: bool,
}

impl Filter<'_> {
    fn removes(&self, name: &str) -> bool {
        if self.rules.allow.iter().any(|r| r.matches(name, self.host)) {
            return false;
        }
        if name == MARKETING_TOKEN && self.keep_marketing_token {
            return false;
        }
        BUILTIN_NAMES.contains(&name)
            || BUILTIN_PREFIXES.iter().any(|p| name.starts_with(p))
            || self.rules.remove.iter().any(|r| r.matches(name, self.host))
    }

    /// `query` without its tracking pairs (and the empty pieces between
    /// them); borrowed when nothing was removed.
    fn pairs<'q>(&self, query: &'q str) -> Cow<'q, str> {
        if !query.split('&').any(|pair| self.removes(&pair_name(pair))) {
            return Cow::Borrowed(query);
        }
        let kept: Vec<&str> = query
            .split('&')
            .filter(|pair| !pair.is_empty() && !self.removes(&pair_name(pair)))
            .collect();
        Cow::Owned(kept.join("&"))
    }

    /// Cleans a fragment written in query syntax (`#a=1&utm_x=2`, or
    /// `#/route?a=1`). Text fragments (`#:~:text=`) are left alone.
    fn fragment<'f>(&self, fragment: &'f str) -> Cow<'f, str> {
        if fragment.contains(":~:") || !fragment.contains('=') {
            return Cow::Borrowed(fragment);
        }
        let (route, query) = match fragment.split_once('?') {
            Some((route, query)) => (Some(route), query),
            None => (None, fragment),
        };
        match (self.pairs(query), route) {
            (Cow::Borrowed(_), _) => Cow::Borrowed(fragment),
            (Cow::Owned(query), None) => Cow::Owned(query),
            (Cow::Owned(query), Some(route)) if query.is_empty() => Cow::Owned(route.to_owned()),
            (Cow::Owned(query), Some(route)) => Cow::Owned(format!("{route}?{query}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clean(text: &str, rules: &TrackingRules) -> String {
        clean_urls(text, rules, &Context::new()).unwrap().text
    }

    #[test]
    fn user_rules_scope_by_domain_and_allow_wins() {
        let rules =
            TrackingRules::new(&["si@youtube.com", "ref_*@amazon.de"], &["gclid@shop.test"]);
        assert_eq!(
            clean(
                "https://m.youtube.com/w?v=1&si=x https://example.com/?si=x",
                &rules
            ),
            "https://m.youtube.com/w?v=1 https://example.com/?si=x"
        );
        assert_eq!(
            clean("https://www.amazon.de/dp/1?ref_=a&th=1", &rules),
            "https://www.amazon.de/dp/1?th=1"
        );
        assert_eq!(
            clean(
                "https://shop.test/?gclid=1 https://other.test/?gclid=1",
                &rules
            ),
            "https://shop.test/?gclid=1 https://other.test/"
        );
        assert_eq!(
            TrackingRules::new(&["a=b", "", "x@"], &[""]),
            TrackingRules::default()
        );
    }

    #[test]
    fn marketing_token_survives_on_unsubscribe_links() {
        let rules = TrackingRules::default();
        assert_eq!(
            clean("https://a.test/unsubscribe?mkt_tok=1", &rules),
            "https://a.test/unsubscribe?mkt_tok=1"
        );
        assert_eq!(
            clean("https://a.test/p?mkt_tok=1", &rules),
            "https://a.test/p"
        );
    }
}
