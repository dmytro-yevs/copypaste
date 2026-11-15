//! JSON format, minify, validate and key sorting, plus string quoting.

pub(crate) mod reader;
pub(crate) mod string;
pub(crate) mod writer;

use reader::{strip_bom, Discard, Reader};
use writer::{Sorter, Writer};

use super::{indent_unit, INDENT};
use crate::catalogue::{Category, Context, Flags, Output, TransformDef};
use crate::error::{TransformError, Warning};
use crate::params::{ParamSpec, Params};

static FORMAT_PARAMS: [ParamSpec; 2] = [INDENT, ParamSpec::flag("sort_keys", false)];
static VALIDATE_PARAMS: [ParamSpec; 1] = [ParamSpec::flag("lenient", false)];
static SORT_PARAMS: [ParamSpec; 1] = [ParamSpec::flag("recursive", true)];

/// Most layers of string encoding Unquote peels off one JSON document.
const MAX_LAYERS: usize = 8;

pub(super) static FORMAT: TransformDef = TransformDef {
    id: "structure.json_format",
    version: 1,
    category: Category::Structure,
    params: &FORMAT_PARAMS,
    flags: Flags::PLAIN,
    modifier: None,
    example: (
        "{\"id\":7,\"price\":1.10,\"tags\":[\"a\"]}",
        "{\n  \"id\": 7,\n  \"price\": 1.10,\n  \"tags\": [\n    \"a\"\n  ]\n}",
    ),
    run: run_format,
};

pub(super) static MINIFY: TransformDef = TransformDef {
    id: "structure.json_minify",
    version: 1,
    category: Category::Structure,
    params: &[],
    flags: Flags::PLAIN,
    modifier: None,
    example: (
        "{\n  \"a\": [1, 2],\n  \"b\": null\n}",
        "{\"a\":[1,2],\"b\":null}",
    ),
    run: run_minify,
};

pub(super) static VALIDATE: TransformDef = TransformDef {
    id: "structure.json_validate",
    version: 1,
    category: Category::Structure,
    params: &VALIDATE_PARAMS,
    flags: Flags {
        can_be_default: false,
        ..Flags::PLAIN
    },
    modifier: None,
    example: ("{\"ok\": true}", "{\"ok\": true}"),
    run: run_validate,
};

pub(super) static SORT_KEYS: TransformDef = TransformDef {
    id: "structure.json_sort_keys",
    version: 1,
    category: Category::Structure,
    params: &SORT_PARAMS,
    flags: Flags::PLAIN,
    modifier: None,
    example: (
        "{\"b\":1,\"a\":{\"d\":2,\"c\":3}}",
        "{\"a\":{\"c\":3,\"d\":2},\"b\":1}",
    ),
    run: run_sort_keys,
};

pub(super) static QUOTE: TransformDef = TransformDef {
    id: "structure.json_quote",
    version: 1,
    category: Category::Structure,
    params: &[],
    flags: Flags::PLAIN,
    modifier: None,
    example: ("Say \"hi\"\nbye", "\"Say \\\"hi\\\"\\nbye\""),
    run: run_quote,
};

pub(super) static UNQUOTE: TransformDef = TransformDef {
    id: "structure.json_unquote",
    version: 1,
    category: Category::Structure,
    params: &[],
    flags: Flags::PLAIN,
    modifier: None,
    example: ("\"{\\\"a\\\":1}\"", "{\"a\":1}"),
    run: run_unquote,
};

/// Reads `input` and writes it with `unit` indentation, or compact.
fn layout(
    input: &str,
    unit: Option<&'static str>,
    sort: Option<bool>,
    ctx: &Context,
) -> Result<Output, TransformError> {
    let text = strip_bom(input);
    let mut reader = Reader::new(text, false);
    let mut writer = Writer::new(unit, text.len());
    match sort {
        None => reader.drive(&mut writer, ctx)?,
        Some(recursive) => {
            let mut sorter = Sorter::new(text, recursive);
            let mut count = 0;
            while let Some(event) = reader.next_event()? {
                ctx.tick(count)?;
                count += 1;
                sorter.push(event);
            }
            sorter.replay(&mut writer, ctx)?;
        }
    }
    Ok(Output {
        text: writer.finish(),
        warnings: reader.into_warnings(),
        sensitive: false,
    })
}

fn run_format(input: &str, params: &Params, ctx: &Context) -> Result<Output, TransformError> {
    let sort = params.flag("sort_keys").then_some(true);
    layout(input, Some(indent_unit(params)), sort, ctx)
}

fn run_minify(input: &str, _: &Params, ctx: &Context) -> Result<Output, TransformError> {
    layout(input, None, None, ctx)
}

fn run_validate(input: &str, params: &Params, ctx: &Context) -> Result<Output, TransformError> {
    let mut reader = Reader::new(strip_bom(input), params.flag("lenient"));
    reader.drive(&mut Discard, ctx)?;
    Ok(Output {
        text: input.to_owned(),
        warnings: reader.into_warnings(),
        sensitive: false,
    })
}

/// The indentation of the first indented line, so sorting keeps the
/// input's layout: none for one-line documents, else a tab, four spaces or
/// two spaces.
fn input_unit(text: &str) -> Option<&'static str> {
    let line = text.lines().skip(1).find(|l| l.starts_with([' ', '\t']))?;
    Some(if line.starts_with('\t') {
        "\t"
    } else if line.starts_with("    ") && !line.starts_with("     ") {
        "    "
    } else {
        "  "
    })
}

fn run_sort_keys(input: &str, params: &Params, ctx: &Context) -> Result<Output, TransformError> {
    layout(
        input,
        input_unit(input),
        Some(params.flag("recursive")),
        ctx,
    )
}

fn run_quote(input: &str, _: &Params, _: &Context) -> Result<Output, TransformError> {
    string::quoted(input).map(Output::new)
}

/// Decodes one string literal, with or without its quotes. When the result
/// is itself a quoted object or array, those layers are decoded too.
fn run_unquote(input: &str, _: &Params, ctx: &Context) -> Result<Output, TransformError> {
    let trimmed = input.trim();
    let quoted = trimmed.len() >= 2 && trimmed.starts_with('"') && trimmed.ends_with('"');
    let (content, base) = if quoted {
        let start = input.len() - input.trim_start().len();
        (&trimmed[1..trimmed.len() - 1], start + 1)
    } else {
        (input, 0)
    };
    if quoted {
        if let Some(at) = string::unescaped_quote(content) {
            return Err(TransformError::invalid_input(
                "json.unescaped_quote",
                input,
                base + at,
            ));
        }
    }
    let mut text = string::unescape(content)
        .map_err(|at| TransformError::invalid_input("json.invalid_escape", input, base + at))?;
    let mut layers = 0;
    while layers < MAX_LAYERS {
        ctx.check()?;
        let Some(inner) = quoted_document(&text) else {
            break;
        };
        text = inner;
        layers += 1;
    }
    let output = Output::new(text);
    Ok(if layers > 0 {
        output.warn(Warning::new("json.double_encoded"))
    } else {
        output
    })
}

/// The decoded body of `text` when it is a string literal holding an
/// object or array.
fn quoted_document(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.len() < 2 || !trimmed.starts_with('"') || !trimmed.ends_with('"') {
        return None;
    }
    let content = &trimmed[1..trimmed.len() - 1];
    if string::unescaped_quote(content).is_some() {
        return None;
    }
    string::unescape(content)
        .ok()
        .filter(|decoded| decoded.trim_start().starts_with(['{', '[']))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorting_keeps_the_input_layout() {
        assert_eq!(input_unit("{\"a\":1}"), None);
        assert_eq!(input_unit("{\n    \"a\": 1\n}"), Some("    "));
        assert_eq!(input_unit("{\n\t\"a\": 1\n}"), Some("\t"));
        assert_eq!(
            input_unit("{\n  \"a\": {\n    \"b\": 1\n  }\n}"),
            Some("  ")
        );
    }
}
