//! The registry of built-in transforms, the step runner and the localized
//! self-description.
//!
//! Definitions are `static` items in their category modules. The registry
//! itself is a list of references built on first use; localized strings are
//! parsed from the embedded `l10n/<locale>.json` only while [`describe`]
//! runs and are not cached.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::error::{check_output_len, TransformError, Warning};
use crate::params::{ParamKind, ParamSpec, Params};
use crate::MAX_INPUT_BYTES;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Structure,
    Encode,
    Case,
    Hash,
    Lines,
    Clean,
    Url,
    Markup,
    Dev,
}

impl Category {
    /// The key of this category's object in `l10n/<locale>.json`.
    pub fn key(self) -> &'static str {
        match self {
            Self::Structure => "structure",
            Self::Encode => "encode",
            Self::Case => "case",
            Self::Hash => "hash",
            Self::Lines => "lines",
            Self::Clean => "clean",
            Self::Url => "url",
            Self::Markup => "markup",
            Self::Dev => "dev",
        }
    }
}

/// Capability flags of a transform.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Flags {
    /// Same input, version and parameters always give the same output.
    pub deterministic: bool,
    /// May serve as a default paste pipeline step.
    pub can_be_default: bool,
    /// Needs a free-text parameter to be useful, so the input method offers
    /// it only through saved pipelines.
    pub needs_free_text: bool,
    /// The output is sensitive whatever the source (decoded tokens).
    pub always_sensitive: bool,
    /// Reads the HTML flavor of a rich item instead of its plain text.
    pub rich_input: bool,
}

impl Flags {
    /// A deterministic plain-text transform usable as a default.
    pub const PLAIN: Self = Self {
        deterministic: true,
        can_be_default: true,
        needs_free_text: false,
        always_sensitive: false,
        rich_input: false,
    };
}

pub(crate) type Runner = fn(&str, &Params, &Context) -> Result<Output, TransformError>;

/// One built-in transform. Category modules declare these as statics and
/// add them in their `register` function.
pub struct TransformDef {
    pub(crate) id: &'static str,
    /// Raised only when the output for the same input and parameters changes.
    pub(crate) version: u32,
    pub(crate) category: Category,
    pub(crate) params: &'static [ParamSpec],
    pub(crate) flags: Flags,
    /// Name under which templates use this transform as `| name`, run with
    /// default parameters. Only deterministic transforms without free-text
    /// parameters get one.
    pub(crate) modifier: Option<&'static str>,
    /// Worked example `(input, output)` with default parameters.
    pub(crate) example: (&'static str, &'static str),
    pub(crate) run: Runner,
}

impl TransformDef {
    pub fn id(&self) -> &'static str {
        self.id
    }

    pub fn version(&self) -> u32 {
        self.version
    }

    pub fn category(&self) -> Category {
        self.category
    }

    pub fn params(&self) -> &'static [ParamSpec] {
        self.params
    }

    pub fn flags(&self) -> Flags {
        self.flags
    }

    pub fn modifier(&self) -> Option<&'static str> {
        self.modifier
    }

    pub fn example(&self) -> (&'static str, &'static str) {
        self.example
    }
}

impl std::fmt::Debug for TransformDef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TransformDef")
            .field("id", &self.id)
            .field("version", &self.version)
            .finish_non_exhaustive()
    }
}

/// Every registered transform, in palette order.
#[derive(Debug, Default)]
pub struct Catalogue {
    defs: Vec<&'static TransformDef>,
}

impl Catalogue {
    pub(crate) fn add(&mut self, def: &'static TransformDef) {
        debug_assert!(self.get(def.id).is_none(), "duplicate transform {}", def.id);
        debug_assert!(
            def.id.starts_with(def.category.key()),
            "transform {} is outside its category",
            def.id
        );
        debug_assert!(
            def.modifier.is_none()
                || (def.flags.deterministic
                    && !def.flags.needs_free_text
                    && !def.flags.always_sensitive),
            "transform {} cannot be a template modifier",
            def.id
        );
        self.defs.push(def);
    }

    pub fn get(&self, id: &str) -> Option<&'static TransformDef> {
        self.defs.iter().copied().find(|def| def.id == id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &'static TransformDef> + '_ {
        self.defs.iter().copied()
    }

    pub fn len(&self) -> usize {
        self.defs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }
}

/// The registry, built on first use.
pub fn catalogue() -> &'static Catalogue {
    static CATALOGUE: OnceLock<Catalogue> = OnceLock::new();
    CATALOGUE.get_or_init(|| {
        let mut catalogue = Catalogue::default();
        crate::structure::register(&mut catalogue);
        crate::encode::register(&mut catalogue);
        crate::case::register(&mut catalogue);
        crate::hash::register(&mut catalogue);
        crate::lines::register(&mut catalogue);
        crate::clean::register(&mut catalogue);
        crate::url::register(&mut catalogue);
        crate::markup::register(&mut catalogue);
        crate::dev::register(&mut catalogue);
        catalogue
    })
}

/// Per-run environment: cancellation, time budget, and the device locale
/// and zone for the few transforms that offer them as an option.
#[derive(Clone, Debug, Default)]
pub struct Context<'a> {
    cancel: Option<&'a AtomicBool>,
    deadline: Option<Instant>,
    locale: Option<String>,
    time_zone: Option<jiff::tz::TimeZone>,
}

impl<'a> Context<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Stops the run with [`TransformError::Cancelled`] once `flag` is set.
    pub fn with_cancel(mut self, flag: &'a AtomicBool) -> Self {
        self.cancel = Some(flag);
        self
    }

    /// Stops the run with [`TransformError::BudgetExceeded`] after `deadline`.
    pub fn with_deadline(mut self, deadline: Instant) -> Self {
        self.deadline = Some(deadline);
        self
    }

    /// BCP 47 tag of the device locale, used only where a parameter asks.
    pub fn with_locale(mut self, locale: impl Into<String>) -> Self {
        self.locale = Some(locale.into());
        self
    }

    /// The device zone, for dates written without an offset.
    pub fn with_time_zone(mut self, zone: jiff::tz::TimeZone) -> Self {
        self.time_zone = Some(zone);
        self
    }

    pub fn locale(&self) -> Option<&str> {
        self.locale.as_deref()
    }

    pub fn time_zone(&self) -> Option<&jiff::tz::TimeZone> {
        self.time_zone.as_ref()
    }

    /// Fails if the run was cancelled or ran out of time.
    pub fn check(&self) -> Result<(), TransformError> {
        if self.cancel.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
            return Err(TransformError::Cancelled);
        }
        if self.deadline.is_some_and(|d| Instant::now() >= d) {
            return Err(TransformError::BudgetExceeded);
        }
        Ok(())
    }

    /// [`Self::check`] once every 1,024 iterations of a loop.
    pub fn tick(&self, iteration: usize) -> Result<(), TransformError> {
        if iteration.is_multiple_of(1024) {
            self.check()
        } else {
            Ok(())
        }
    }
}

/// A transform with its parameters, as stored in a pipeline.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Step {
    pub transform_id: String,
    pub version: u32,
    #[serde(default)]
    pub params: Map<String, Value>,
    /// Honoured by chain runners; [`apply`] runs the step either way.
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
}

fn enabled_by_default() -> bool {
    true
}

impl Step {
    pub fn new(transform_id: impl Into<String>, version: u32) -> Self {
        Self {
            transform_id: transform_id.into(),
            version,
            params: Map::new(),
            enabled: true,
        }
    }

    /// A step at the current version of a registered transform.
    pub fn current(transform_id: &str) -> Option<Self> {
        catalogue()
            .get(transform_id)
            .map(|def| Self::new(def.id, def.version))
    }

    pub fn param(mut self, name: &str, value: impl Into<Value>) -> Self {
        self.params.insert(name.to_owned(), value.into());
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Output {
    pub text: String,
    pub warnings: Vec<Warning>,
    /// Set for transforms whose output is sensitive whatever the source.
    pub sensitive: bool,
}

impl Output {
    pub fn new(text: String) -> Self {
        Self {
            text,
            ..Self::default()
        }
    }

    pub fn warn(mut self, warning: Warning) -> Self {
        self.warnings.push(warning);
        self
    }
}

fn resolve(step: &Step) -> Result<(&'static TransformDef, Params), TransformError> {
    let def =
        catalogue()
            .get(&step.transform_id)
            .ok_or_else(|| TransformError::UnknownTransform {
                id: step.transform_id.clone(),
            })?;
    if step.version != def.version {
        return Err(TransformError::UnsupportedVersion {
            id: step.transform_id.clone(),
            version: step.version,
            current: def.version,
        });
    }
    Ok((def, Params::resolve(def.params, &step.params)?))
}

/// Checks that a step names a known transform at a runnable version with
/// valid parameters, without running it.
pub fn validate_step(step: &Step) -> Result<(), TransformError> {
    resolve(step).map(|_| ())
}

/// Runs one step over `input`.
pub fn apply(step: &Step, input: &str, ctx: &Context) -> Result<Output, TransformError> {
    if input.len() > MAX_INPUT_BYTES {
        return Err(TransformError::InputTooLarge {
            bytes: input.len(),
            limit: MAX_INPUT_BYTES,
        });
    }
    let (def, params) = resolve(step)?;
    ctx.check()?;
    let mut output = (def.run)(input, &params, ctx)?;
    check_output_len(output.text.len())?;
    output.sensitive |= def.flags.always_sensitive;
    Ok(output)
}

/// Locales with an embedded catalogue translation.
pub const LOCALES: [&str; 12] = [
    "de", "en", "es", "fr", "it", "ja", "ko", "pl", "pt", "tr", "uk", "zh",
];

fn locale_source(locale: &str) -> &'static str {
    match locale {
        "de" => include_str!("../l10n/de.json"),
        "es" => include_str!("../l10n/es.json"),
        "fr" => include_str!("../l10n/fr.json"),
        "it" => include_str!("../l10n/it.json"),
        "ja" => include_str!("../l10n/ja.json"),
        "ko" => include_str!("../l10n/ko.json"),
        "pl" => include_str!("../l10n/pl.json"),
        "pt" => include_str!("../l10n/pt.json"),
        "tr" => include_str!("../l10n/tr.json"),
        "uk" => include_str!("../l10n/uk.json"),
        "zh" => include_str!("../l10n/zh.json"),
        _ => include_str!("../l10n/en.json"),
    }
}

/// The language subtag of `tag` if a translation exists, else `en`.
fn language(tag: &str) -> &'static str {
    let primary = tag.split(['-', '_']).next().unwrap_or_default();
    LOCALES
        .into_iter()
        .find(|l| l.eq_ignore_ascii_case(primary))
        .unwrap_or("en")
}

#[derive(Debug, Default, Deserialize)]
struct EntryText {
    name: String,
    description: String,
    #[serde(default)]
    keywords: Vec<String>,
    #[serde(default)]
    params: HashMap<String, ParamText>,
}

#[derive(Debug, Default, Deserialize)]
struct ParamText {
    label: String,
    #[serde(default)]
    options: HashMap<String, String>,
}

/// Category key, then transform id.
type Strings = HashMap<String, HashMap<String, EntryText>>;

fn strings(locale: &str) -> Strings {
    serde_json::from_str(locale_source(locale)).unwrap_or_default()
}

fn entry<'s>(strings: &'s Strings, def: &TransformDef) -> Option<&'s EntryText> {
    strings.get(def.category.key())?.get(def.id)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OptionInfo {
    pub value: &'static str,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ParamInfo {
    pub name: &'static str,
    pub label: String,
    pub kind: ParamKind,
    /// Labels of a choice's options, in schema order.
    pub options: Vec<OptionInfo>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TransformInfo {
    pub id: &'static str,
    pub version: u32,
    pub category: Category,
    pub name: String,
    pub description: String,
    /// Localized keywords followed by the English ones, for search.
    pub keywords: Vec<String>,
    pub example_in: &'static str,
    pub example_out: &'static str,
    pub params: Vec<ParamInfo>,
    pub flags: Flags,
    /// Template modifier name, if the transform has one.
    pub modifier: Option<&'static str>,
}

/// The localized catalogue for a BCP 47 `locale`, falling back to English
/// per string.
pub fn describe(locale: &str) -> Vec<TransformInfo> {
    let language = language(locale);
    let local = strings(language);
    let english = if language == "en" {
        Strings::new()
    } else {
        strings("en")
    };
    catalogue()
        .iter()
        .map(|def| info(def, entry(&local, def), entry(&english, def)))
        .collect()
}

fn info(def: &TransformDef, local: Option<&EntryText>, en: Option<&EntryText>) -> TransformInfo {
    let pick = |get: fn(&EntryText) -> &str| {
        [local, en]
            .into_iter()
            .flatten()
            .map(get)
            .find(|s| !s.is_empty())
            .map(str::to_owned)
    };
    let mut keywords: Vec<String> = Vec::new();
    for keyword in [local, en].into_iter().flatten().flat_map(|e| &e.keywords) {
        if !keywords.contains(keyword) {
            keywords.push(keyword.clone());
        }
    }
    TransformInfo {
        id: def.id,
        version: def.version,
        category: def.category,
        name: pick(|e| &e.name).unwrap_or_else(|| def.id.to_owned()),
        description: pick(|e| &e.description).unwrap_or_default(),
        keywords,
        example_in: def.example.0,
        example_out: def.example.1,
        params: def
            .params
            .iter()
            .map(|spec| param_info(spec, local, en))
            .collect(),
        flags: def.flags,
        modifier: def.modifier,
    }
}

fn param_info(spec: &ParamSpec, local: Option<&EntryText>, en: Option<&EntryText>) -> ParamInfo {
    let texts: Vec<&ParamText> = [local, en]
        .into_iter()
        .flatten()
        .filter_map(|e| e.params.get(spec.name))
        .collect();
    let label = texts
        .iter()
        .map(|t| t.label.as_str())
        .find(|l| !l.is_empty())
        .unwrap_or(spec.name)
        .to_owned();
    let options = match spec.kind {
        ParamKind::Choice { options, .. } => options
            .iter()
            .map(|value| OptionInfo {
                value,
                label: texts
                    .iter()
                    .find_map(|t| t.options.get(*value).filter(|l| !l.is_empty()))
                    .map_or_else(|| (*value).to_owned(), Clone::clone),
            })
            .collect(),
        _ => Vec::new(),
    };
    ParamInfo {
        name: spec.name,
        label,
        kind: spec.kind,
        options,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_falls_back_to_english() {
        assert_eq!(language("pt-BR"), "pt");
        assert_eq!(language("zh_Hans_CN"), "zh");
        assert_eq!(language("UK"), "uk");
        assert_eq!(language("nl"), "en");
    }

    #[test]
    fn every_locale_file_parses_and_has_every_category() {
        let categories = [
            Category::Structure,
            Category::Encode,
            Category::Case,
            Category::Hash,
            Category::Lines,
            Category::Clean,
            Category::Url,
            Category::Markup,
            Category::Dev,
        ];
        for locale in LOCALES {
            let parsed: Strings = serde_json::from_str(locale_source(locale))
                .unwrap_or_else(|e| panic!("l10n/{locale}.json: {e}"));
            for category in categories {
                assert!(parsed.contains_key(category.key()), "{locale} {category:?}");
            }
        }
    }

    #[test]
    fn apply_refuses_oversized_input_and_old_versions() {
        let big = "a".repeat(MAX_INPUT_BYTES + 1);
        let step = Step::current("case.upper").unwrap();
        assert!(matches!(
            apply(&step, &big, &Context::new()),
            Err(TransformError::InputTooLarge { .. })
        ));
        let newer = Step::new("case.upper", 99);
        assert_eq!(
            validate_step(&newer).unwrap_err().code(),
            "unsupported_version"
        );
        assert_eq!(
            validate_step(&Step::new("future.thing", 1))
                .unwrap_err()
                .code(),
            "unknown_transform"
        );
    }

    #[test]
    fn a_set_cancel_flag_stops_the_run() {
        let flag = AtomicBool::new(true);
        let ctx = Context::new().with_cancel(&flag);
        let step = Step::current("case.upper").unwrap();
        assert_eq!(apply(&step, "abc", &ctx), Err(TransformError::Cancelled));
    }
}
