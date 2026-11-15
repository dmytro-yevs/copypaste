//! Template rendering, fields and lint over the library.
//!
//! A body is parsed once per revision and kept in a small cache. Nested
//! `{{snippet abbr=…}}` placeholders resolve against the library, `| name`
//! modifiers are the deterministic transforms, and `{{clipboard}}` reads the
//! user's last copied text from the host only when a placeholder asks.
//! Without history access, `clipboard index=N` for N ≥ 1 renders empty.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::SystemTime;

use copypaste_module_sdk::{ClipboardText, ModuleHostRequest};
use copypaste_template::{
    Issue, Lint, LintContext, Modifiers, NestedBody, NestedSnippet, Origin, RenderContext,
    RenderError, Required, Severity, SnippetRef, SnippetResolver, Template, TemplateField,
};

use crate::host::{self, Host};
use crate::library::Library;
use crate::model::Body;
use crate::strings::Strings;

/// Most parsed templates kept between renders.
const CACHE_ENTRIES: usize = 32;

/// Parsed bodies keyed by body revision; the source is compared on a hit,
/// so a stale entry is never used.
#[derive(Default)]
pub(crate) struct Templates(RefCell<Vec<(String, Arc<Template>)>>);

impl Templates {
    pub(crate) fn parsed(&self, revision: Option<&str>, body: &str) -> Arc<Template> {
        let Some(revision) = revision else {
            return Arc::new(copypaste_template::parse(body));
        };
        let mut entries = self.0.borrow_mut();
        if let Some(at) = entries
            .iter()
            .position(|(cached, template)| cached == revision && template.source() == body)
        {
            let entry = entries.remove(at);
            let template = Arc::clone(&entry.1);
            entries.push(entry);
            return template;
        }
        let template = Arc::new(copypaste_template::parse(body));
        if entries.len() >= CACHE_ENTRIES {
            entries.remove(0);
        }
        entries.push((revision.to_owned(), Arc::clone(&template)));
        template
    }
}

/// Where `{{clipboard}}` reads from.
pub(crate) enum Clipboard<'a> {
    /// Nothing: clipboard placeholders stay empty.
    None,
    /// The text the user last copied, read from the host when asked.
    Host(&'a dyn Host),
}

/// One expansion of a body.
pub(crate) struct Expansion<'a> {
    pub(crate) body: &'a str,
    pub(crate) revision: Option<&'a str>,
    pub(crate) template: bool,
    pub(crate) origin: Option<Origin>,
    pub(crate) concealed: bool,
    pub(crate) values: &'a BTreeMap<String, String>,
    pub(crate) clipboard: Clipboard<'a>,
    pub(crate) locale: &'a str,
}

pub(crate) struct Rendering {
    pub(crate) text: String,
    /// Caret steps back from the end of `text` to the first `{{cursor}}`.
    pub(crate) cursor_graphemes_from_end: Option<u32>,
    pub(crate) warnings: Vec<Issue>,
    pub(crate) missing_required: Vec<Required>,
    /// A concealed snippet or secret clipboard text went into the text.
    pub(crate) secret: bool,
}

impl Rendering {
    /// Required fields `values` left empty.
    pub(crate) fn missing_fields(&self) -> Vec<String> {
        self.missing_required
            .iter()
            .filter_map(|required| match required {
                Required::Field { name } => Some(name.clone()),
                Required::Clipboard { .. } => None,
            })
            .collect()
    }

    /// A clipboard placeholder was left empty.
    pub(crate) fn reads_clipboard(&self) -> bool {
        self.warnings
            .iter()
            .any(|issue| matches!(issue, Issue::EmptyClipboard { .. }))
    }
}

/// The expansion of a stored snippet.
pub(crate) fn stored<'a>(
    id: &str,
    library: &Library,
    body: &'a Body,
    values: &'a BTreeMap<String, String>,
    clipboard: Clipboard<'a>,
    locale: &'a str,
) -> Option<Expansion<'a>> {
    let meta = library.meta(id)?;
    Some(Expansion {
        body: body.content(),
        revision: Some(&body.revision),
        template: meta.template,
        origin: Some(Origin {
            id: id.to_owned(),
            abbreviation: meta.abbreviation.clone(),
        }),
        concealed: meta.concealed,
        values,
        clipboard,
        locale,
    })
}

impl Expansion<'_> {
    /// Expands once. With Template off the body is the output, as written.
    pub(crate) fn render(
        &self,
        library: &Library,
        templates: &Templates,
    ) -> Result<Rendering, RenderError> {
        if !self.template {
            return Ok(Rendering {
                text: self.body.to_owned(),
                cursor_graphemes_from_end: None,
                warnings: Vec::new(),
                missing_required: Vec::new(),
                secret: self.concealed,
            });
        }
        let template = templates.parsed(self.revision, self.body);
        let resolver = Resolver::new(library, templates);
        let secret = Cell::new(self.concealed);
        let clipboard = |index: u8| -> Option<String> {
            let Clipboard::Host(host) = self.clipboard else {
                return None;
            };
            if index != 0 {
                return None;
            }
            let read: ClipboardText = host::request(host, ModuleHostRequest::ClipboardRead).ok()?;
            if read.secret {
                secret.set(true);
            }
            read.text
        };
        let zone = local_zone();
        let rendered = copypaste_template::render(
            &template,
            &RenderContext {
                now: SystemTime::now(),
                local_zone: &zone,
                locale: self.locale,
                values: self.values,
                clipboard: &clipboard,
                snippets: &resolver,
                modifiers: &TransformModifiers,
                origin: self.origin.as_ref(),
            },
        )?;
        Ok(Rendering {
            cursor_graphemes_from_end: rendered
                .cursor_graphemes_from_end()
                .map(|steps| u32::try_from(steps).unwrap_or(u32::MAX)),
            secret: secret.get() || resolver.concealed.get(),
            text: rendered.text,
            warnings: rendered.warnings,
            missing_required: rendered.missing_required,
        })
    }

    /// The fill-in form of the template, nested snippets included.
    pub(crate) fn fields(&self, library: &Library, templates: &Templates) -> Vec<TemplateField> {
        if !self.template {
            return Vec::new();
        }
        let template = templates.parsed(self.revision, self.body);
        copypaste_template::fields(
            &template,
            self.origin.as_ref(),
            &Resolver::new(library, templates),
        )
    }

    pub(crate) fn lint(&self, library: &Library, templates: &Templates) -> Vec<Lint> {
        let template = templates.parsed(self.revision, self.body);
        copypaste_template::lint(
            &template,
            self.template,
            &LintContext {
                locale: self.locale,
                snippets: &Resolver::new(library, templates),
                modifiers: &TransformModifiers,
                origin: self.origin.as_ref(),
            },
        )
    }
}

/// The user-facing text of a lint issue.
pub(crate) fn issue_message(strings: &Strings, issue: &Issue) -> String {
    let value = serde_json::to_value(issue).unwrap_or_default();
    let code = value["code"].as_str().unwrap_or("malformed");
    let mut camel = String::with_capacity(code.len());
    let mut upper = false;
    for char in code.chars() {
        if char == '_' {
            upper = true;
        } else if upper {
            camel.extend(char.to_uppercase());
            upper = false;
        } else {
            camel.push(char);
        }
    }
    let detail = value
        .as_object()
        .and_then(|fields| fields.iter().find(|(key, _)| key.as_str() != "code"))
        .map(|(_, argument)| match argument {
            serde_json::Value::String(text) => text.clone(),
            serde_json::Value::Array(chain) => chain
                .iter()
                .filter_map(serde_json::Value::as_str)
                .collect::<Vec<_>>()
                .join(" → "),
            other => other.to_string(),
        })
        .unwrap_or_default();
    strings.select("snippetsLint", &camel, &[("detail", &detail)])
}

/// Lint messages of at least `severity`, in body order.
pub(crate) fn lint_messages(strings: &Strings, lints: &[Lint], severity: Severity) -> Vec<String> {
    lints
        .iter()
        .filter(|lint| lint.severity >= severity)
        .map(|lint| issue_message(strings, &lint.issue))
        .collect()
}

/// The device zone as an IANA id; `UTC` when it cannot be named.
fn local_zone() -> String {
    jiff::tz::TimeZone::try_system()
        .ok()
        .and_then(|zone| zone.iana_name().map(str::to_owned))
        .unwrap_or_else(|| "UTC".to_owned())
}

/// Live snippets for `snippet` placeholders, looked up once per reference
/// in one expansion.
struct Resolver<'a> {
    library: &'a Library,
    templates: &'a Templates,
    resolved: RefCell<Vec<(SnippetRef, Option<NestedSnippet>)>>,
    /// A concealed snippet was inserted.
    concealed: Cell<bool>,
}

impl<'a> Resolver<'a> {
    fn new(library: &'a Library, templates: &'a Templates) -> Self {
        Self {
            library,
            templates,
            resolved: RefCell::default(),
            concealed: Cell::new(false),
        }
    }

    fn lookup(&self, reference: &SnippetRef) -> Option<(NestedSnippet, bool)> {
        let id = match reference {
            SnippetRef::Id(id) => id.clone(),
            SnippetRef::Abbreviation(abbreviation) => {
                self.library.resolve_abbreviation(abbreviation)?
            }
        };
        let meta = self.library.live(&id).ok()?;
        let body = self.library.body(&id).ok().flatten()?;
        let nested = if meta.template {
            NestedBody::Template(self.templates.parsed(Some(&body.revision), body.content()))
        } else {
            NestedBody::Literal(body.content().to_owned())
        };
        Some((
            NestedSnippet {
                origin: Origin {
                    id,
                    abbreviation: meta.abbreviation.clone(),
                },
                body: nested,
            },
            meta.concealed,
        ))
    }
}

impl SnippetResolver for Resolver<'_> {
    fn resolve(&self, reference: &SnippetRef) -> Option<NestedSnippet> {
        if let Some((_, found)) = self
            .resolved
            .borrow()
            .iter()
            .find(|(known, _)| known == reference)
        {
            return found.clone();
        }
        let found = self.lookup(reference).map(|(nested, concealed)| {
            if concealed {
                self.concealed.set(true);
            }
            nested
        });
        self.resolved
            .borrow_mut()
            .push((reference.clone(), found.clone()));
        found
    }
}

/// Template modifiers are the deterministic text transforms that name one.
struct TransformModifiers;

impl Modifiers for TransformModifiers {
    fn contains(&self, id: &str) -> bool {
        copypaste_transform::modifiers().any(|modifier| modifier.name == id)
    }

    fn apply(&self, id: &str, input: &str) -> Option<String> {
        copypaste_transform::apply_modifier(id, input).ok()
    }
}
