//! CSV and TSV to JSON. Fields stay strings; nothing is type-guessed.

use std::collections::HashSet;

use csv::{ReaderBuilder, StringRecord};

use super::json::reader::{strip_bom, Container, Event, Sink};
use super::json::string::quoted;
use super::json::writer::Writer;
use crate::catalogue::{Category, Context, Flags, Output, TransformDef};
use crate::error::{TransformError, Warning};
use crate::params::{ParamSpec, Params};

static PARAMS: [ParamSpec; 2] = [
    ParamSpec::flag("header", true),
    ParamSpec::choice("delimiter", &["auto", "comma", "semicolon", "tab"], "auto"),
];

pub(super) static CSV_TO_JSON: TransformDef = TransformDef {
    id: "structure.csv_to_json",
    version: 1,
    category: Category::Structure,
    params: &PARAMS,
    flags: Flags::PLAIN,
    modifier: None,
    example: (
        "name,age\nAda,36",
        "[\n  {\n    \"name\": \"Ada\",\n    \"age\": \"36\"\n  }\n]",
    ),
    run: run_csv_to_json,
};

/// The separator used most often on the first record, outside quotes:
/// comma unless a tab or semicolon is more frequent.
fn detect_delimiter(text: &str) -> u8 {
    const CANDIDATES: [u8; 3] = [b',', b'\t', b';'];
    let mut counts = [0usize; 3];
    let mut in_quotes = false;
    for byte in text.bytes() {
        match byte {
            b'"' => in_quotes = !in_quotes,
            b'\n' if !in_quotes => break,
            _ if !in_quotes => {
                if let Some(i) = CANDIDATES.iter().position(|&c| c == byte) {
                    counts[i] += 1;
                }
            }
            _ => {}
        }
    }
    let best =
        (1..CANDIDATES.len()).fold(0, |best, i| if counts[i] > counts[best] { i } else { best });
    CANDIDATES[best]
}

/// The offset of a quote that opens a field and is never closed. The CSV
/// reader would silently take the rest of the text as that field.
fn unclosed_quote(text: &str, delimiter: u8) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut field_start = true;
    let mut i = 0;
    while i < bytes.len() {
        let byte = bytes[i];
        if byte == b'"' && field_start {
            let open = i;
            i += 1;
            loop {
                match bytes.get(i) {
                    None => return Some(open),
                    Some(b'"') if bytes.get(i + 1) == Some(&b'"') => i += 2,
                    Some(b'"') => break,
                    Some(_) => i += 1,
                }
            }
            field_start = false;
        } else {
            field_start = matches!(byte, b'\n' | b'\r') || byte == delimiter;
        }
        i += 1;
    }
    None
}

fn run_csv_to_json(input: &str, params: &Params, ctx: &Context) -> Result<Output, TransformError> {
    let text = strip_bom(input);
    let delimiter = match params.choice("delimiter") {
        "comma" => b',',
        "semicolon" => b';',
        "tab" => b'\t',
        _ => detect_delimiter(text),
    };
    if let Some(at) = unclosed_quote(text, delimiter) {
        return Err(TransformError::invalid_input(
            "csv.unclosed_quote",
            text,
            at,
        ));
    }
    let mut reader = ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .delimiter(delimiter)
        .from_reader(text.as_bytes());
    let mut record = StringRecord::new();
    let mut names: Option<Vec<String>> = None;
    let mut warnings = Vec::new();
    let mut writer = Writer::new(Some("  "), text.len());
    writer.event(Event::Open(Container::Array))?;
    let mut row = 0;
    loop {
        let offset = reader.position().byte() as usize;
        let more = reader
            .read_record(&mut record)
            .map_err(|_| TransformError::invalid_input("csv.syntax", text, offset))?;
        if !more {
            break;
        }
        ctx.tick(row)?;
        row += 1;
        if params.flag("header") && names.is_none() {
            let mut seen = HashSet::new();
            if record.iter().any(|name| !seen.insert(name)) {
                warnings.push(Warning::new("csv.duplicate_header"));
            }
            names = Some(record.iter().map(quoted).collect::<Result<_, _>>()?);
            continue;
        }
        match &names {
            Some(names) => {
                if record.len() != names.len() {
                    let at = record.position().map_or(offset, |p| p.byte() as usize);
                    return Err(TransformError::invalid_input("csv.field_count", text, at));
                }
                writer.event(Event::Open(Container::Object))?;
                for (name, field) in names.iter().zip(record.iter()) {
                    writer.event(Event::Key(name))?;
                    writer.event(Event::Value(&quoted(field)?))?;
                }
                writer.event(Event::Close(Container::Object))?;
            }
            None => {
                writer.event(Event::Open(Container::Array))?;
                for field in record.iter() {
                    writer.event(Event::Value(&quoted(field)?))?;
                }
                writer.event(Event::Close(Container::Array))?;
            }
        }
    }
    writer.event(Event::Close(Container::Array))?;
    Ok(Output {
        text: writer.finish(),
        warnings,
        sensitive: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_most_frequent_separator_on_the_first_record_wins() {
        assert_eq!(detect_delimiter("a;b;c\n1,2"), b';');
        assert_eq!(detect_delimiter("a\tb\n"), b'\t');
        assert_eq!(detect_delimiter("\"x;y\",b\n"), b',');
        assert_eq!(detect_delimiter("single"), b',');
    }
}
