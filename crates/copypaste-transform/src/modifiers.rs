//! Template modifiers: the transforms a definition names with a modifier,
//! each run with its default parameters.

use serde::Serialize;

use crate::catalogue::{apply, catalogue, Context, Step};
use crate::error::TransformError;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Modifier {
    /// The name written after `|` in a template placeholder.
    pub name: &'static str,
    pub transform_id: &'static str,
}

/// Every modifier, in catalogue order.
pub fn modifiers() -> impl Iterator<Item = Modifier> {
    catalogue().iter().filter_map(|def| {
        def.modifier().map(|name| Modifier {
            name,
            transform_id: def.id(),
        })
    })
}

/// Runs the modifier `name` over `input`.
pub fn apply_modifier(name: &str, input: &str) -> Result<String, TransformError> {
    let def = catalogue()
        .iter()
        .find(|def| def.modifier() == Some(name))
        .ok_or_else(|| TransformError::UnknownModifier {
            name: name.to_owned(),
        })?;
    apply(&Step::new(def.id(), def.version()), input, &Context::new()).map(|output| output.text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifier_names_are_unique_and_chain_left_to_right() {
        let names: Vec<_> = modifiers().map(|m| m.name).collect();
        let mut unique = names.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(names.len(), unique.len());

        let trimmed = apply_modifier("trim", "  ab ").unwrap();
        assert_eq!(apply_modifier("upper", &trimmed).unwrap(), "AB");
        assert_eq!(
            apply_modifier("shout", "x").unwrap_err().code(),
            "unknown_modifier"
        );
    }
}
