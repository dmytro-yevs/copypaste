//! Nested snippet references: lookup through the caller and the walk guard
//! that stops cycles and limits depth.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::lint::Issue;
use crate::syntax::{Directive, Params, Template};
use crate::{MAX_DEPTH, MAX_NESTED_EXPANSIONS};

/// How a `snippet` placeholder names its target.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnippetRef {
    Abbreviation(String),
    Id(String),
}

impl SnippetRef {
    pub(crate) fn parse(params: &mut Params) -> Result<Self, Issue> {
        match (params.take("abbr"), params.take("id")) {
            (Some(_), Some(_)) => Err(Issue::ConflictingParameters),
            (Some(abbr), None) => Ok(Self::Abbreviation(abbr)),
            (None, Some(id)) => Ok(Self::Id(id)),
            (None, None) => Err(Issue::MissingParameter { key: "abbr".into() }),
        }
    }
}

impl Template {
    /// The body with every well-formed top-level `snippet abbr=…` reference
    /// whose abbreviation `matches` renamed to `to`, keeping its modifiers.
    /// `None` when no reference matched.
    pub fn rename_snippet_references(
        &self,
        matches: &dyn Fn(&str) -> bool,
        to: &str,
    ) -> Option<String> {
        let source = self.source();
        let mut out = String::with_capacity(source.len());
        let mut copied = 0;
        let mut renamed = false;
        for placeholder in self.placeholders() {
            let Ok(parsed) = &placeholder.parsed else {
                continue;
            };
            match &parsed.directive {
                Directive::Snippet(SnippetRef::Abbreviation(abbreviation))
                    if matches(abbreviation) => {}
                _ => continue,
            }
            out.push_str(&source[copied..placeholder.span.start]);
            out.push_str("{{snippet abbr=\"");
            for c in to.chars() {
                if matches!(c, '"' | '\\') {
                    out.push('\\');
                }
                out.push(c);
            }
            out.push('"');
            for modifier in &parsed.modifiers {
                out.push_str(" | ");
                out.push_str(modifier);
            }
            out.push_str("}}");
            copied = placeholder.span.end;
            renamed = true;
        }
        renamed.then(|| {
            out.push_str(&source[copied..]);
            out
        })
    }
}

/// The snippet being expanded or linted, so a reference back to it is a cycle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Origin {
    pub id: String,
    pub abbreviation: Option<String>,
}

impl Origin {
    fn label(&self) -> String {
        self.abbreviation.clone().unwrap_or_else(|| self.id.clone())
    }
}

/// A live (not deleted) snippet returned by the resolver.
#[derive(Clone, Debug)]
pub struct NestedSnippet {
    pub origin: Origin,
    pub body: NestedBody,
}

/// A nested body is inserted literally when its Template switch is off.
/// Templates are shared so the caller's parse cache covers nested bodies.
#[derive(Clone, Debug)]
pub enum NestedBody {
    Literal(String),
    Template(Arc<Template>),
}

/// Looks up snippets for `snippet` placeholders. Deleted, trashed and unknown
/// snippets resolve to `None`.
pub trait SnippetResolver {
    fn resolve(&self, reference: &SnippetRef) -> Option<NestedSnippet>;
}

/// A resolver that knows no snippets.
pub struct NoSnippets;

impl SnippetResolver for NoSnippets {
    fn resolve(&self, _reference: &SnippetRef) -> Option<NestedSnippet> {
        None
    }
}

/// The chain of snippets currently being expanded, outermost first.
pub(crate) struct Chain {
    stack: Vec<Origin>,
    has_root: bool,
    /// Nested bodies entered so far in this walk, at any depth.
    entered: usize,
}

impl Chain {
    pub fn new(root: Option<&Origin>) -> Self {
        Self {
            stack: root.into_iter().cloned().collect(),
            has_root: root.is_some(),
            entered: 0,
        }
    }

    /// Nesting depth of the body being walked; the top level is 0.
    pub fn depth(&self) -> usize {
        self.stack.len() - usize::from(self.has_root)
    }

    /// Resolves a reference met at the current depth and enters it. The
    /// caller must call [`Chain::leave`] after walking the returned body.
    pub fn enter(
        &mut self,
        reference: &SnippetRef,
        resolver: &dyn SnippetResolver,
    ) -> Result<NestedBody, Issue> {
        if self.depth() >= MAX_DEPTH {
            return Err(Issue::DepthExceeded);
        }
        if self.entered >= MAX_NESTED_EXPANSIONS {
            return Err(Issue::NestingLimit);
        }
        let nested = resolver.resolve(reference).ok_or(Issue::UnknownSnippet)?;
        if let Some(start) = self.stack.iter().position(|o| o.id == nested.origin.id) {
            let mut chain: Vec<String> = self.stack[start..].iter().map(Origin::label).collect();
            chain.push(nested.origin.label());
            return Err(Issue::Cycle { chain });
        }
        self.entered += 1;
        self.stack.push(nested.origin);
        Ok(nested.body)
    }

    pub fn leave(&mut self) {
        self.stack.pop();
    }
}

#[cfg(test)]
mod tests {
    use crate::parse;

    #[test]
    fn renaming_rewrites_only_matching_references_and_keeps_modifiers() {
        let body = r#"a {{snippet abbr=sig | upper}} \{{snippet abbr=sig}} {{snippet abbr=x}}"#;
        let renamed = parse(body)
            .rename_snippet_references(&|abbr| abbr == "sig", "s\"2")
            .unwrap();
        assert_eq!(
            renamed,
            r#"a {{snippet abbr="s\"2" | upper}} \{{snippet abbr=sig}} {{snippet abbr=x}}"#
        );
        assert!(parse(body)
            .rename_snippet_references(&|abbr| abbr == "none", "y")
            .is_none());
    }
}
