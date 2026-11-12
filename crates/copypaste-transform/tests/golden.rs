//! Golden corpus: every `tests/golden/<category>.json` case runs through the
//! public API and must match byte for byte on every host.
//!
//! A case is `{transform, params?, locale?, input, output}` or, for a
//! refusal, `{transform, params?, input, error}` where `error` is the
//! stable error code. `warnings` (codes) and `sensitive` are checked when
//! present.

use std::fs;
use std::path::{Path, PathBuf};

use copypaste_transform::{apply, catalogue, describe, Context, Step, LOCALES};
use serde::Deserialize;
use serde_json::{Map, Value};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    transform: String,
    #[serde(default)]
    params: Map<String, Value>,
    locale: Option<String>,
    input: String,
    output: Option<String>,
    error: Option<String>,
    warnings: Option<Vec<String>>,
    sensitive: Option<bool>,
}

fn corpus_files() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|entry| entry.expect("corpus entry").path())
        .filter(|path| path.extension().is_some_and(|e| e == "json"))
        .collect();
    files.sort();
    files
}

/// The mismatch for one case, if any.
fn check(case: &Case) -> Option<String> {
    let Some(def) = catalogue().get(&case.transform) else {
        return Some(format!("{} is not registered", case.transform));
    };
    let mut step = Step::new(def.id(), def.version());
    step.params = case.params.clone();
    let ctx = match &case.locale {
        Some(locale) => Context::new().with_locale(locale.clone()),
        None => Context::new(),
    };
    match (apply(&step, &case.input, &ctx), &case.output, &case.error) {
        (Ok(out), Some(expected), None) => {
            let warnings: Vec<&str> = out.warnings.iter().map(|w| w.code).collect();
            if out.text != *expected {
                Some(format!("got {:?}, want {expected:?}", out.text))
            } else if case.warnings.as_ref().is_some_and(|w| *w != warnings) {
                Some(format!("warnings {warnings:?}, want {:?}", case.warnings))
            } else if case.sensitive.is_some_and(|s| s != out.sensitive) {
                Some(format!(
                    "sensitive {}, want {:?}",
                    out.sensitive, case.sensitive
                ))
            } else {
                None
            }
        }
        (Err(err), None, Some(code)) if err.code() == code => None,
        (result, output, error) => Some(format!(
            "got {:?}, want output {output:?} or error {error:?}",
            result.map(|o| o.text).map_err(|e| e.code())
        )),
    }
}

#[test]
fn golden_corpus_matches() {
    let mut total = 0;
    let mut failures = Vec::new();
    for file in corpus_files() {
        let name = file
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let text = fs::read_to_string(&file).unwrap_or_else(|e| panic!("{name}: {e}"));
        let cases: Vec<Case> =
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(!cases.is_empty(), "{name} has no cases");
        for (index, case) in cases.iter().enumerate() {
            assert!(
                case.output.is_some() != case.error.is_some(),
                "{name}[{index}] needs exactly one of output and error"
            );
            if let Some(failure) = check(case) {
                failures.push(format!("{name}[{index}] {}: {failure}", case.transform));
            }
        }
        total += cases.len();
    }
    assert!(total > 0, "the golden corpus is empty");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Every registered transform names, describes and demonstrates itself in
/// every locale, and its worked example is what it really produces.
#[test]
fn catalogue_describes_every_transform() {
    for def in catalogue().iter() {
        let (input, expected) = def.example();
        let step = Step::new(def.id(), def.version());
        let output = apply(&step, input, &Context::new())
            .unwrap_or_else(|e| panic!("{} example fails: {e}", def.id()));
        assert_eq!(output.text, expected, "{} example", def.id());
    }
    let english = describe("en");
    for locale in LOCALES {
        for (info, en) in describe(locale).iter().zip(&english) {
            let id = info.id;
            assert!(!info.description.is_empty(), "{locale} {id} description");
            assert!(info.name != id, "{locale} {id} has no name");
            if locale != "en" {
                assert_ne!(
                    info.description, en.description,
                    "{locale} {id} untranslated"
                );
            }
            for param in &info.params {
                assert!(
                    param.label != param.name,
                    "{locale} {id}.{} label",
                    param.name
                );
                for option in &param.options {
                    assert!(
                        option.label != option.value,
                        "{locale} {id}.{}.{} label",
                        param.name,
                        option.value
                    );
                }
            }
        }
    }
}
