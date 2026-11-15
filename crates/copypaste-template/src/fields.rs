//! Fill-in fields: the `field` directive and the per-expansion registry that
//! merges fields by name and caps their number.

use serde::{Deserialize, Serialize};

use crate::lint::Issue;
use crate::nested::{Chain, NestedBody, Origin, SnippetResolver};
use crate::syntax::{Directive, Params, Template};
use crate::{MAX_CHOICE_OPTIONS, MAX_FIELDS, MAX_FIELD_NAME_CHARS};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldKind {
    Text,
    Multiline,
    Choice,
}

/// One input of the fill-in form.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateField {
    pub name: String,
    pub label: String,
    pub kind: FieldKind,
    /// Choice options; empty for other kinds.
    pub options: Vec<String>,
    /// Pre-filled value. A choice field always has one: its first option
    /// when the template names none or names one that is not an option.
    pub default: Option<String>,
    pub required: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct FieldSpec {
    pub name: String,
    pub label: Option<String>,
    pub kind: FieldKind,
    pub options: Vec<String>,
    /// The default as written, before the choice fallback.
    pub default: Option<String>,
    pub required: Option<bool>,
}

impl FieldSpec {
    pub fn parse(params: &mut Params) -> Result<Self, Issue> {
        let name = params
            .take("name")
            .ok_or_else(|| Issue::MissingParameter { key: "name".into() })?;
        let name_chars = name.chars().count();
        if name.trim().is_empty() || name_chars > MAX_FIELD_NAME_CHARS {
            return Err(Issue::InvalidValue { key: "name".into() });
        }
        let kind = match params.take("type").as_deref() {
            None | Some("text") => FieldKind::Text,
            Some("multiline") => FieldKind::Multiline,
            Some("choice") => FieldKind::Choice,
            Some(_) => return Err(Issue::InvalidValue { key: "type".into() }),
        };
        let options = match kind {
            FieldKind::Choice => {
                let raw = params
                    .take("options")
                    .ok_or_else(|| Issue::MissingParameter {
                        key: "options".into(),
                    })?;
                let options = split_options(&raw);
                if options.is_empty()
                    || options.len() > MAX_CHOICE_OPTIONS
                    || options.iter().any(String::is_empty)
                {
                    return Err(Issue::InvalidValue {
                        key: "options".into(),
                    });
                }
                options
            }
            _ => Vec::new(),
        };
        Ok(Self {
            name,
            label: params.take("label"),
            kind,
            options,
            default: params.take("default"),
            required: params.take_bool("required")?,
        })
    }

    /// The default actually used: a choice falls back to its first option.
    pub fn effective_default(&self) -> Option<&str> {
        match self.kind {
            FieldKind::Choice => self
                .default
                .as_deref()
                .filter(|d| self.options.iter().any(|o| o == d))
                .or(self.options.first().map(String::as_str)),
            _ => self.default.as_deref(),
        }
    }

    /// A field is required unless it says otherwise or has a default.
    pub fn required(&self) -> bool {
        self.required.unwrap_or(self.default.is_none())
    }

    pub fn default_is_invalid(&self) -> bool {
        self.kind == FieldKind::Choice
            && self.effective_default() != self.default.as_deref()
            && self.default.is_some()
    }

    fn conflicts_with(&self, other: &FieldSpec) -> bool {
        self.kind != other.kind
            || self.options != other.options
            || self.default != other.default
            || self.required != other.required
            || self.label != other.label
    }

    fn to_field(&self) -> TemplateField {
        TemplateField {
            name: self.name.clone(),
            label: self.label.clone().unwrap_or_else(|| self.name.clone()),
            kind: self.kind,
            options: self.options.clone(),
            default: self.effective_default().map(str::to_owned),
            required: self.required(),
        }
    }
}

/// Splits `options` on commas; `\,` is a literal comma. Options are trimmed.
fn split_options(raw: &str) -> Vec<String> {
    let mut options = Vec::new();
    let mut current = String::new();
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&',') => {
                current.push(',');
                chars.next();
            }
            ',' => options.push(std::mem::take(&mut current).trim().to_owned()),
            _ => current.push(c),
        }
    }
    options.push(current.trim().to_owned());
    options
}

/// Outcome of registering a field occurrence.
pub(crate) enum Registered {
    /// The first definition of this name, possibly just registered.
    Known(TemplateField),
    /// A later definition that differs from the first; the first is used.
    Conflicting(TemplateField),
    /// The name would be field number `MAX_FIELDS + 1` or later.
    OverLimit,
}

/// Distinct fields of one expansion, in first-occurrence order.
#[derive(Default)]
pub(crate) struct FieldRegistry {
    fields: Vec<FieldSpec>,
}

impl FieldRegistry {
    pub fn register(&mut self, spec: &FieldSpec) -> Registered {
        if let Some(first) = self.fields.iter().find(|f| f.name == spec.name) {
            return if first.conflicts_with(spec) {
                Registered::Conflicting(first.to_field())
            } else {
                Registered::Known(first.to_field())
            };
        }
        if self.fields.len() >= MAX_FIELDS {
            return Registered::OverLimit;
        }
        self.fields.push(spec.clone());
        Registered::Known(spec.to_field())
    }

    pub fn into_fields(self) -> Vec<TemplateField> {
        self.fields.iter().map(FieldSpec::to_field).collect()
    }
}

/// The fill-in form of a template: at most [`MAX_FIELDS`] distinct fields in
/// body order, nested snippets included (keyed by name).
pub fn fields(
    template: &Template,
    origin: Option<&Origin>,
    snippets: &dyn SnippetResolver,
) -> Vec<TemplateField> {
    let mut registry = FieldRegistry::default();
    collect(template, &mut Chain::new(origin), snippets, &mut registry);
    registry.into_fields()
}

fn collect(
    template: &Template,
    chain: &mut Chain,
    snippets: &dyn SnippetResolver,
    registry: &mut FieldRegistry,
) {
    for placeholder in template.placeholders() {
        match placeholder.parsed.as_ref().map(|p| &p.directive) {
            Ok(Directive::Field(spec)) => {
                registry.register(spec);
            }
            Ok(Directive::Snippet(reference)) => {
                if let Ok(body) = chain.enter(reference, snippets) {
                    if let NestedBody::Template(nested) = body {
                        collect(&nested, chain, snippets, registry);
                    }
                    chain.leave();
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_split_on_unescaped_commas() {
        assert_eq!(split_options(r"a, b\,c ,d"), vec!["a", "b,c", "d"]);
    }
}
