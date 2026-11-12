use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use copypaste_template::{
    fields, lint, parse, render, Issue, LintContext, Modifiers, NestedBody, NestedSnippet,
    NoModifiers, NoSnippets, Origin, RenderContext, RenderError, Rendered, Required, SnippetRef,
    SnippetResolver, MAX_FIELDS, MAX_NESTED_EXPANSIONS,
};

const KYIV: &str = "Europe/Kyiv";

/// Snippets by abbreviation; every body is a template.
#[derive(Default)]
struct Library(HashMap<String, String>);

impl Library {
    fn with(mut self, abbr: &str, body: &str) -> Self {
        self.0.insert(abbr.into(), body.into());
        self
    }
}

impl SnippetResolver for Library {
    fn resolve(&self, reference: &SnippetRef) -> Option<NestedSnippet> {
        let SnippetRef::Abbreviation(abbr) = reference else {
            return None;
        };
        let body = self.0.get(abbr)?;
        Some(NestedSnippet {
            origin: Origin {
                id: format!("id-{abbr}"),
                abbreviation: Some(abbr.clone()),
            },
            body: NestedBody::Template(Arc::new(parse(body))),
        })
    }
}

struct CaseModifiers;

impl Modifiers for CaseModifiers {
    fn contains(&self, id: &str) -> bool {
        matches!(id, "trim" | "upper")
    }

    fn apply(&self, id: &str, input: &str) -> Option<String> {
        match id {
            "trim" => Some(input.trim().to_owned()),
            "upper" => Some(input.to_uppercase()),
            _ => None,
        }
    }
}

/// Seconds since the Unix epoch for an RFC 3339 instant.
fn at(rfc3339: &str) -> SystemTime {
    let seconds: i64 = rfc3339
        .parse::<jiff::Timestamp>()
        .expect("instant")
        .as_second();
    SystemTime::UNIX_EPOCH + Duration::from_secs(seconds.try_into().expect("after epoch"))
}

struct Setup<'a> {
    now: SystemTime,
    clipboard: Option<&'a str>,
    values: BTreeMap<String, String>,
    snippets: &'a dyn SnippetResolver,
    modifiers: &'a dyn Modifiers,
    origin: Option<Origin>,
}

impl<'a> Setup<'a> {
    fn new() -> Self {
        Self {
            now: at("2026-01-31T08:00:00Z"),
            clipboard: None,
            values: BTreeMap::new(),
            snippets: &NoSnippets,
            modifiers: &NoModifiers,
            origin: None,
        }
    }

    fn try_render(&self, body: &str) -> Result<Rendered, RenderError> {
        let clipboard = |index: u8| (index == 0).then(|| self.clipboard.map(str::to_owned))?;
        render(
            &parse(body),
            &RenderContext {
                now: self.now,
                local_zone: KYIV,
                locale: "en",
                values: &self.values,
                clipboard: &clipboard,
                snippets: self.snippets,
                modifiers: self.modifiers,
                origin: self.origin.as_ref(),
            },
        )
    }

    fn render(&self, body: &str) -> Rendered {
        self.try_render(body).expect("rendered")
    }

    fn lint(&self, body: &str) -> Vec<Issue> {
        lint(
            &parse(body),
            true,
            &LintContext {
                locale: "en",
                snippets: self.snippets,
                modifiers: self.modifiers,
                origin: self.origin.as_ref(),
            },
        )
        .into_iter()
        .map(|lint| lint.issue)
        .collect()
    }
}

#[test]
fn escapes_render_literal_braces() {
    let rendered = Setup::new().render(r"\{{date}} and \}} and \x");
    assert_eq!(rendered.text, r"{{date}} and }} and \x");
    assert_eq!(rendered.unfilled, 0);
}

#[test]
fn unknown_placeholder_stays_verbatim_with_one_warning() {
    let setup = Setup::new();
    let rendered = setup.render("a {{foo}} b");
    assert_eq!(rendered.text, "a {{foo}} b");
    assert_eq!(rendered.unfilled, 1);
    assert_eq!(
        setup.lint("a {{foo}} b"),
        vec![Issue::UnknownPlaceholder { name: "foo".into() }]
    );
    let off = lint(
        &parse("{{date}}"),
        false,
        &LintContext {
            locale: "en",
            snippets: &NoSnippets,
            modifiers: &NoModifiers,
            origin: None,
        },
    );
    assert_eq!(off.len(), 1);
    assert_eq!(off[0].issue, Issue::ContainsPlaceholders);
}

#[test]
fn month_offset_clamps_to_the_last_day_in_the_device_zone() {
    let mut setup = Setup::new();
    setup.now = at("2026-01-31T10:00:00+02:00");
    let rendered = setup.render(r#"{{date format="yyyy-MM-dd" offset=+1mo}}"#);
    assert_eq!(rendered.text, "2026-02-28");
    let named = setup.render(r#"{{date format="EEEE, d MMMM" locale=uk}}"#);
    assert_eq!(named.text, "субота, 31 січня");
    let week = setup.render(r#"{{date format="'W'ww, EEE"}}"#);
    assert_eq!(week.text, "W05, Sat");
}

#[test]
fn clock_units_are_absolute_and_day_units_keep_the_wall_clock() {
    let mut setup = Setup::new();
    // Kyiv moves from +02:00 to +03:00 at 03:00 on 2026-03-29.
    setup.now = at("2026-03-29T02:30:00+02:00");
    let hours = setup.render(r#"{{datetime format="yyyy-MM-dd HH:mm" offset=+2h}}"#);
    assert_eq!(hours.text, "2026-03-29 05:30");
    setup.now = at("2026-03-28T10:00:00+02:00");
    let day = setup.render(r#"{{datetime format="yyyy-MM-dd HH:mm" offset=+1d}}"#);
    assert_eq!(day.text, "2026-03-29 10:00");
}

#[test]
fn nested_dates_share_the_captured_instant() {
    let library = Library::default().with("inner", r#"{{time format="HH:mm:ss"}}"#);
    let mut setup = Setup::new();
    setup.snippets = &library;
    let rendered = setup.render(r#"{{time format="HH:mm:ss"}}|{{snippet abbr=inner}}"#);
    assert_eq!(rendered.text, "10:00:00|10:00:00");
}

#[test]
fn clipboard_text_is_never_parsed_again() {
    let mut setup = Setup::new();
    setup.clipboard = Some("{{uuid}}");
    assert_eq!(setup.render("[{{clipboard}}]").text, "[{{uuid}}]");
    setup.clipboard = None;
    let empty = setup.render("{{clipboard required=true}}");
    assert_eq!(empty.text, "");
    assert_eq!(empty.warnings, vec![Issue::EmptyClipboard { index: 0 }]);
    assert_eq!(
        empty.missing_required,
        vec![Required::Clipboard { index: 0 }]
    );
}

#[test]
fn fields_with_one_name_share_one_input() {
    let body = r#"{{field name=who}} and {{field name="who"}}"#;
    let form = fields(&parse(body), None, &NoSnippets);
    assert_eq!(form.len(), 1);
    assert!(form[0].required);
    let mut setup = Setup::new();
    assert_eq!(
        setup.render(body).missing_required,
        vec![Required::Field { name: "who".into() }]
    );
    setup.values.insert("who".into(), "Ann".into());
    assert_eq!(setup.render(body).text, "Ann and Ann");
}

#[test]
fn an_eleventh_field_is_linted_and_left_verbatim() {
    let body: String = (1..=MAX_FIELDS + 1)
        .map(|n| format!("{{{{field name=f{n} default=v}}}}"))
        .collect();
    let setup = Setup::new();
    assert_eq!(
        setup.lint(&body),
        vec![Issue::TooManyFields { name: "f11".into() }]
    );
    let rendered = setup.render(&body);
    assert_eq!(
        rendered.text,
        format!("{}{{{{field name=f11 default=v}}}}", "v".repeat(10))
    );
    assert_eq!(rendered.unfilled, 1);
    assert_eq!(fields(&parse(&body), None, &NoSnippets).len(), MAX_FIELDS);
}

#[test]
fn cycles_and_depth_four_stay_verbatim() {
    let library = Library::default()
        .with("a", "A({{snippet abbr=b}})")
        .with("b", "B({{snippet abbr=a}})")
        .with("one", "1{{snippet abbr=two}}")
        .with("two", "2{{snippet abbr=three}}")
        .with("three", "3{{snippet abbr=four}}")
        .with("four", "4");
    let mut setup = Setup::new();
    setup.snippets = &library;
    setup.origin = Some(Origin {
        id: "id-a".into(),
        abbreviation: Some("a".into()),
    });
    let rendered = setup.render("A({{snippet abbr=b}})");
    assert_eq!(rendered.text, "A(B({{snippet abbr=a}}))");
    assert_eq!(
        setup.lint("A({{snippet abbr=b}})"),
        vec![Issue::Cycle {
            chain: vec!["a".into(), "b".into(), "a".into()],
        }]
    );
    setup.origin = None;
    let deep = setup.render("{{snippet abbr=one}}");
    assert_eq!(deep.text, "123{{snippet abbr=four}}");
    assert_eq!(deep.warnings, vec![Issue::DepthExceeded]);
}

#[test]
fn nested_insertions_are_bounded_per_expansion() {
    let library = Library::default().with("leaf", ".").with(
        "many",
        &"{{snippet abbr=leaf}}".repeat(MAX_NESTED_EXPANSIONS),
    );
    let mut setup = Setup::new();
    setup.snippets = &library;
    let rendered = setup.render("{{snippet abbr=many}}{{snippet abbr=leaf}}");
    assert_eq!(
        rendered.text,
        format!(
            "{}{}",
            ".".repeat(MAX_NESTED_EXPANSIONS - 1),
            "{{snippet abbr=leaf}}".repeat(2)
        )
    );
    assert_eq!(rendered.warnings, vec![Issue::NestingLimit; 2]);
}

#[test]
fn only_the_first_cursor_counts() {
    let setup = Setup::new();
    let body = "a👍\r\n{{cursor}}b{{cursor}}c";
    assert_eq!(setup.lint(body), vec![Issue::ExtraCursor]);
    let rendered = setup.render(body);
    assert_eq!(rendered.text, "a👍\r\nbc");
    assert_eq!(rendered.cursor_graphemes_from_end(), Some(2));
}

#[test]
fn modifiers_apply_left_to_right_and_unknown_ones_keep_the_placeholder() {
    let mut setup = Setup::new();
    setup.modifiers = &CaseModifiers;
    setup.clipboard = Some("  ab ");
    assert_eq!(setup.render("{{clipboard | trim | upper}}").text, "AB");
    let unknown = setup.render("{{clipboard | trim | shout}}");
    assert_eq!(unknown.text, "{{clipboard | trim | shout}}");
    assert_eq!(
        setup.lint("{{clipboard | trim | shout}}"),
        vec![Issue::UnknownModifier { id: "shout".into() }]
    );
}

#[test]
fn large_template_renders_quickly_and_oversized_output_is_refused() {
    let mut setup = Setup::new();
    let mut body = String::new();
    for n in 0..MAX_FIELDS {
        setup.values.insert(format!("f{n}"), "value".into());
        body.push_str(&format!("{{{{field name=f{n}}}}} {{{{date}}}} "));
    }
    while body.len() < 64 * 1024 - 32 {
        body.push_str("lorem ipsum {{uuid}} ");
    }
    let started = Instant::now();
    let rendered = setup.render(&body);
    eprintln!(
        "64 KiB template with {MAX_FIELDS} fields rendered in {:?}",
        started.elapsed()
    );
    assert_eq!(rendered.unfilled, 0);

    let huge = "x".repeat(600 * 1024);
    setup.clipboard = Some(&huge);
    assert_eq!(
        setup.try_render("{{clipboard}}{{clipboard}}"),
        Err(RenderError::OutputTooLarge)
    );
}
