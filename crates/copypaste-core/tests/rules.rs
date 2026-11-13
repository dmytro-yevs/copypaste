use std::time::{Duration, Instant};

use copypaste_core::rules::compile::{
    validate, CompiledRuleSet, RuleErrorCode, RuleHealth, RuleInput,
};
use copypaste_core::rules::evaluate::{
    EvalFlags, EvaluationContext, EvaluationInput, EvaluationLimits,
};
use copypaste_core::rules::trace::{ConditionResult, RuleResult};
use copypaste_core::storage::organization::{ContentRuleRecord, RuleOrigin};

const DEVICE: &str = "device-a";
const T0: i64 = 1_700_000_000_000;

fn rule(id: &str, order: &str, definition: &str) -> RuleInput {
    RuleInput {
        rule: ContentRuleRecord {
            id: id.into(),
            origin: RuleOrigin::User,
            builtin_version: None,
            customized_fields: Vec::new(),
            name: id.into(),
            definition_json: definition.into(),
            device_scope: Vec::new(),
            schema_version: 1,
            effective_from: T0,
            deleted_at: None,
            base_stamp: None,
        },
        enabled: true,
        order_key: order.into(),
    }
}

fn set(rules: Vec<RuleInput>) -> CompiledRuleSet {
    CompiledRuleSet::compile(rules, DEVICE)
}

fn capture(rules: &CompiledRuleSet, text: &str) -> copypaste_core::rules::evaluate::RuleOutcome {
    rules.evaluate(&EvaluationInput::text(text, T0), EvaluationContext::Capture)
}

const CONTAINS_ALPHA: &str = r#"{"type":"content_text","op":"contains","value":"alpha"}"#;
const CONTAINS_BETA: &str = r#"{"type":"content_text","op":"contains","value":"beta"}"#;

#[test]
fn any_matches_on_one_condition_and_all_needs_every_condition() {
    let any = format!(
        r#"{{"match":"any","conditions":[{CONTAINS_ALPHA},{CONTAINS_BETA}],"actions":[{{"type":"mark_secret"}}]}}"#
    );
    let all = any.replace(r#""match":"any""#, r#""match":"all""#);
    assert!(capture(&set(vec![rule("r", "a", &any)]), "ALPHA only").secret);
    assert!(!capture(&set(vec![rule("r", "a", &all)]), "ALPHA only").secret);
    assert!(capture(&set(vec![rule("r", "a", &all)]), "alpha and beta").secret);
}

#[test]
fn shortest_expiry_wins_regardless_of_order() {
    let expire = |ms: i64| {
        format!(
            r#"{{"match":"all","conditions":[{CONTAINS_ALPHA}],"actions":[{{"type":"delete_after","duration_ms":{ms}}}]}}"#
        )
    };
    for (first, second) in [(3_600_000, 600_000), (600_000, 3_600_000)] {
        let rules = set(vec![
            rule("r1", "a", &expire(first)),
            rule("r2", "b", &expire(second)),
        ]);
        assert_eq!(
            capture(&rules, "alpha").expiry.unwrap().duration_ms,
            600_000
        );
    }
}

#[test]
fn receipt_runs_only_protection_for_items_created_after_effective_from() {
    let definition = format!(
        r#"{{"match":"all","conditions":[{CONTAINS_ALPHA}],"actions":[{{"type":"mark_secret"}},
        {{"type":"transform","transform_id":"trim"}},{{"type":"delete_after","duration_ms":60000}}]}}"#
    );
    let rules = set(vec![rule("r", "a", &definition)]);
    let mut old = EvaluationInput::text("alpha", T0 - 1);
    let evaluation = rules.evaluate_traced(&old, EvaluationContext::Receipt);
    assert!(evaluation.outcome.is_noop());
    assert_eq!(
        evaluation.trace.unwrap().rules[0].result,
        RuleResult::NotEffective
    );
    old.created_at = T0 - 1;
    assert!(rules.evaluate(&old, EvaluationContext::Capture).secret);

    let fresh = EvaluationInput::text("alpha", T0);
    let received = rules.evaluate(&fresh, EvaluationContext::Receipt);
    assert!(received.secret && received.transforms.is_empty());
    assert_eq!(
        received.protection_subset().expiry.unwrap().duration_ms,
        60_000
    );
}

#[test]
fn stop_processing_prevents_later_tags_and_hides_later_rules_from_the_trace() {
    let first = format!(
        r#"{{"match":"all","conditions":[{CONTAINS_ALPHA}],"actions":[{{"type":"hide_from_ai"}}],"stop_processing":true}}"#
    );
    let second = format!(
        r#"{{"match":"all","conditions":[{CONTAINS_ALPHA}],"actions":[{{"type":"add_tags","tag_ids":["t"]}}]}}"#
    );
    let rules = set(vec![
        rule("second", "b", &second),
        rule("first", "a", &first),
    ]);
    let evaluation = rules.evaluate_traced(
        &EvaluationInput::text("alpha", T0),
        EvaluationContext::Capture,
    );
    assert!(evaluation.outcome.hide_from_ai && evaluation.outcome.tag_ids.is_empty());
    let trace = evaluation.trace.unwrap();
    assert_eq!(trace.rules.len(), 1);
    assert_eq!(trace.rules[0].rule_id, "first");
}

#[test]
fn transforms_chain_in_rule_order_against_the_original_content() {
    let transform = |id: &str, condition: &str| {
        format!(
            r#"{{"match":"all","conditions":[{condition}],"actions":[{{"type":"transform","transform_id":"{id}"}}]}}"#
        )
    };
    // The first transform would remove "alpha"; the second rule still sees it.
    let rules = set(vec![
        rule("upper", "a", &transform("remove_alpha", CONTAINS_ALPHA)),
        rule("lower", "b", &transform("lowercase", CONTAINS_ALPHA)),
    ]);
    let outcome = capture(&rules, "alpha");
    let chain: Vec<_> = outcome
        .transforms
        .iter()
        .map(|step| step.transform_id.as_str())
        .collect();
    assert_eq!(chain, ["remove_alpha", "lowercase"]);
}

#[test]
fn editor_rejects_look_around_backreferences_and_long_patterns() {
    let regex = |pattern: &str| {
        format!(
            r#"{{"match":"all","conditions":[{{"type":"content_text","op":"matches_regex","value":{}}}],"actions":[{{"type":"mark_secret"}}]}}"#,
            serde_json::to_string(pattern).unwrap()
        )
    };
    let codes = |pattern: &str| {
        validate(&regex(pattern))
            .into_iter()
            .map(|e| e.code)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        codes("pass(?=word)"),
        [RuleErrorCode::UnsupportedLookAround]
    );
    assert_eq!(codes(r"(\w)\1"), [RuleErrorCode::UnsupportedBackreference]);
    assert_eq!(codes(&"a".repeat(2_001)), [RuleErrorCode::PatternTooLong]);
    assert!(codes(&"a".repeat(2_000)).is_empty());
    assert_eq!(validate(&regex("pass(?=word)"))[0].offset, Some(4));
}

#[test]
fn a_rule_that_cannot_compile_here_fails_closed() {
    let synced = r#"{"match":"all","conditions":[{"type":"content_text","op":"matches_regex","value":"(a{1,100}){1,100}"}],"actions":[{"type":"add_tags","tag_ids":["t"]}]}"#;
    let rules = set(vec![rule("synced", "a", synced)]);
    assert_eq!(rules.status("synced").unwrap().health, RuleHealth::Broken);
    let outcome = capture(&rules, "anything");
    assert!(outcome.secret && outcome.hide_from_ai && outcome.tag_ids.is_empty());
    assert!(outcome.flags.contains(EvalFlags::RULES_INCOMPLETE));
}

#[test]
fn pathological_pattern_on_one_mebibyte_stays_within_budget() {
    // Neither nested repetition matches, so both scan the whole prefix before the
    // last condition matches.
    let definition = r#"{"match":"any","conditions":[{"type":"content_text","op":"matches_regex","value":"(a+)+$"},{"type":"content_text","op":"matches_regex","value":"(x*)*y"},{"type":"content_text","op":"ends_with","value":"!"}],"actions":[{"type":"mark_secret"}]}"#;
    let rules = set(vec![rule("slow", "a", definition)]);
    let text = format!("{}!", "a".repeat(1024 * 1024));
    // Unoptimized test builds run the engine an order of magnitude slower.
    let budget = if cfg!(debug_assertions) {
        Duration::from_millis(2_000)
    } else {
        Duration::from_millis(50)
    };
    let limits = EvaluationLimits {
        budget,
        ..EvaluationLimits::default()
    };
    let started = Instant::now();
    let evaluation = rules.evaluate_with(
        &EvaluationInput::text(&text, T0),
        EvaluationContext::Capture,
        limits,
        false,
    );
    assert!(started.elapsed() < budget);
    assert!(!evaluation
        .outcome
        .flags
        .contains(EvalFlags::BUDGET_EXCEEDED));
    assert!(evaluation.outcome.secret);
}

#[test]
fn unknown_source_app_matches_neither_polarity() {
    let source = |negate: bool| {
        format!(
            r#"{{"match":"all","conditions":[{{"type":"source_app","any_of":["com.example.vault"],"negate":{negate}}}],"actions":[{{"type":"mark_secret"}}]}}"#
        )
    };
    for negate in [false, true] {
        let rules = set(vec![rule("app", "a", &source(negate))]);
        let evaluation =
            rules.evaluate_traced(&EvaluationInput::text("x", T0), EvaluationContext::Capture);
        assert!(!evaluation.outcome.secret);
        let trace = evaluation.trace.unwrap();
        assert_eq!(
            trace.rules[0].conditions[0].result,
            ConditionResult::Unknown
        );
    }
    let mut known = EvaluationInput::text("x", T0);
    known.source_app = Some("COM.EXAMPLE.VAULT");
    assert!(
        set(vec![rule("app", "a", &source(false))])
            .evaluate(&known, EvaluationContext::Capture)
            .secret
    );
}

#[test]
fn unknown_condition_types_are_never_partially_evaluated() {
    let unknown = r#"{"type":"window_title","op":"contains","value":"bank"}"#;
    let tagging = format!(
        r#"{{"match":"any","conditions":[{CONTAINS_ALPHA},{unknown}],"actions":[{{"type":"add_tags","tag_ids":["t"]}}]}}"#
    );
    let rules = set(vec![rule("newer", "a", &tagging)]);
    assert_eq!(
        rules.status("newer").unwrap().health,
        RuleHealth::NeedsUpdate
    );
    let outcome = capture(&rules, "alpha");
    assert!(outcome.is_noop());
    assert!(outcome.flags.contains(EvalFlags::NEEDS_UPDATE));

    // A purely protective rule fails closed when its known part could match.
    let protective = format!(
        r#"{{"match":"all","conditions":[{CONTAINS_ALPHA},{unknown}],"actions":[{{"type":"mark_secret"}}]}}"#
    );
    let rules = set(vec![rule("newer", "a", &protective)]);
    assert!(capture(&rules, "alpha")
        .flags
        .contains(EvalFlags::RULES_INCOMPLETE));
    assert!(capture(&rules, "gamma").is_noop());
}

#[test]
fn conflicting_and_duplicate_actions_are_rejected() {
    let conflicting = r#"{"match":"all","actions":[{"type":"dont_save"},{"type":"mark_secret"}]}"#;
    assert!(validate(conflicting)
        .iter()
        .any(|error| error.code == RuleErrorCode::ConflictingActions));
    let duplicate = r#"{"match":"all","actions":[{"type":"mark_secret"},{"type":"mark_secret"}]}"#;
    assert_eq!(validate(duplicate)[0].code, RuleErrorCode::DuplicateAction);
    let rules = set(vec![rule("bad", "a", conflicting)]);
    assert_eq!(rules.status("bad").unwrap().health, RuleHealth::Broken);
}

#[test]
fn dont_save_is_terminal_at_capture_and_ignored_on_receipt() {
    let skip = format!(
        r#"{{"match":"all","conditions":[{CONTAINS_ALPHA}],"actions":[{{"type":"dont_save"}}]}}"#
    );
    let tag = format!(
        r#"{{"match":"all","conditions":[{CONTAINS_ALPHA}],"actions":[{{"type":"add_tags","tag_ids":["t"]}}]}}"#
    );
    let rules = set(vec![rule("skip", "a", &skip), rule("tag", "b", &tag)]);
    let captured = capture(&rules, "alpha");
    assert_eq!(captured.dont_save.unwrap().rule_id, "skip");
    assert!(captured.tag_ids.is_empty());
    let received = rules.evaluate(
        &EvaluationInput::text("alpha", T0),
        EvaluationContext::Receipt,
    );
    assert!(received.dont_save.is_none());
    assert_eq!(received.tag_ids, ["t"]);
}
