//! Typed parameter schemas and their resolution against a step's JSON
//! object. A missing or `null` value takes the default; anything else that
//! does not fit the schema, including an unknown key, is refused.

use serde::Serialize;
use serde_json::{Map, Value};

use crate::error::{ParamIssue, TransformError};

/// Longest free-text parameter value.
pub const MAX_TEXT_PARAM_BYTES: usize = 4 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct ParamSpec {
    pub name: &'static str,
    pub kind: ParamKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ParamKind {
    Bool {
        default: bool,
    },
    Choice {
        options: &'static [&'static str],
        default: &'static str,
    },
    Int {
        min: i64,
        max: i64,
        default: i64,
    },
    /// Free text of at most `max_bytes` (never above [`MAX_TEXT_PARAM_BYTES`]).
    Text {
        max_bytes: usize,
        default: &'static str,
    },
}

impl ParamSpec {
    pub const fn flag(name: &'static str, default: bool) -> Self {
        Self {
            name,
            kind: ParamKind::Bool { default },
        }
    }

    pub const fn choice(
        name: &'static str,
        options: &'static [&'static str],
        default: &'static str,
    ) -> Self {
        Self {
            name,
            kind: ParamKind::Choice { options, default },
        }
    }

    pub const fn int(name: &'static str, min: i64, max: i64, default: i64) -> Self {
        Self {
            name,
            kind: ParamKind::Int { min, max, default },
        }
    }

    pub const fn text(name: &'static str, default: &'static str) -> Self {
        Self {
            name,
            kind: ParamKind::Text {
                max_bytes: MAX_TEXT_PARAM_BYTES,
                default,
            },
        }
    }

    fn default_value(&self) -> Resolved {
        match self.kind {
            ParamKind::Bool { default } => Resolved::Bool(default),
            ParamKind::Choice { default, .. } => Resolved::Choice(default),
            ParamKind::Int { default, .. } => Resolved::Int(default),
            ParamKind::Text { default, .. } => Resolved::Text(default.to_owned()),
        }
    }

    fn resolve(&self, value: &Value) -> Result<Resolved, ParamIssue> {
        match (self.kind, value) {
            (ParamKind::Bool { .. }, Value::Bool(b)) => Ok(Resolved::Bool(*b)),
            (ParamKind::Choice { options, .. }, Value::String(s)) => options
                .iter()
                .find(|option| **option == s.as_str())
                .map(|option| Resolved::Choice(option))
                .ok_or(ParamIssue::NotAnOption),
            (ParamKind::Int { min, max, .. }, Value::Number(n)) => match n.as_i64() {
                Some(i) if (min..=max).contains(&i) => Ok(Resolved::Int(i)),
                Some(_) => Err(ParamIssue::OutOfRange),
                None => Err(ParamIssue::WrongType),
            },
            (ParamKind::Text { max_bytes, .. }, Value::String(s)) => {
                if s.len() > max_bytes.min(MAX_TEXT_PARAM_BYTES) {
                    Err(ParamIssue::TooLong)
                } else {
                    Ok(Resolved::Text(s.clone()))
                }
            }
            _ => Err(ParamIssue::WrongType),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Resolved {
    Bool(bool),
    Choice(&'static str),
    Int(i64),
    Text(String),
}

/// A step's parameters after validation, one value per schema entry.
#[derive(Clone, Debug, PartialEq)]
pub struct Params {
    values: Vec<(&'static str, Resolved)>,
}

impl Params {
    pub(crate) fn resolve(
        specs: &'static [ParamSpec],
        raw: &Map<String, Value>,
    ) -> Result<Self, TransformError> {
        if let Some(unknown) = raw.keys().find(|k| !specs.iter().any(|s| s.name == *k)) {
            return Err(TransformError::InvalidParam {
                name: unknown.clone(),
                issue: ParamIssue::Unknown,
            });
        }
        let values = specs
            .iter()
            .map(|spec| {
                let value = match raw.get(spec.name) {
                    None | Some(Value::Null) => Ok(spec.default_value()),
                    Some(value) => spec.resolve(value),
                };
                value
                    .map(|v| (spec.name, v))
                    .map_err(|issue| TransformError::InvalidParam {
                        name: spec.name.to_owned(),
                        issue,
                    })
            })
            .collect::<Result<_, _>>()?;
        Ok(Self { values })
    }

    fn get(&self, name: &str) -> Option<&Resolved> {
        let value = self.values.iter().find(|(n, _)| *n == name).map(|(_, v)| v);
        debug_assert!(value.is_some(), "parameter {name} is not in the schema");
        value
    }

    pub fn flag(&self, name: &str) -> bool {
        matches!(self.get(name), Some(Resolved::Bool(true)))
    }

    pub fn choice(&self, name: &str) -> &'static str {
        match self.get(name) {
            Some(Resolved::Choice(c)) => c,
            _ => "",
        }
    }

    pub fn int(&self, name: &str) -> i64 {
        match self.get(name) {
            Some(Resolved::Int(i)) => *i,
            _ => 0,
        }
    }

    pub fn text(&self, name: &str) -> &str {
        match self.get(name) {
            Some(Resolved::Text(t)) => t,
            _ => "",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    static SPECS: [ParamSpec; 3] = [
        ParamSpec::flag("trim", false),
        ParamSpec::choice("order", &["asc", "desc"], "asc"),
        ParamSpec::int("width", 1, 8, 2),
    ];

    fn raw(value: Value) -> Map<String, Value> {
        value.as_object().cloned().unwrap()
    }

    #[test]
    fn defaults_fill_missing_and_null_values() {
        let params = Params::resolve(&SPECS, &raw(json!({"order": null, "width": 4}))).unwrap();
        assert!(!params.flag("trim"));
        assert_eq!(params.choice("order"), "asc");
        assert_eq!(params.int("width"), 4);
    }

    #[test]
    fn unknown_keys_and_bad_values_are_refused() {
        let issue = |v| match Params::resolve(&SPECS, &raw(v)) {
            Err(TransformError::InvalidParam { issue, .. }) => issue,
            other => panic!("expected an invalid parameter, got {other:?}"),
        };
        assert_eq!(issue(json!({"extra": 1})), ParamIssue::Unknown);
        assert_eq!(issue(json!({"order": "up"})), ParamIssue::NotAnOption);
        assert_eq!(issue(json!({"width": 9})), ParamIssue::OutOfRange);
        assert_eq!(issue(json!({"trim": "yes"})), ParamIssue::WrongType);
    }
}
