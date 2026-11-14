//! Content rules at capture, before anything is stored.
//!
//! The host asks for a decision, ingests the plan's content and metadata with
//! its effects (the core writes them in the storing transaction), then calls
//! [`Runtime::finish_capture`]. Transforms run here because the core never
//! links them. Rules never run for history adds, imports, restores, module
//! captures or CopyPaste's own clipboard writes, which never come here. Logs
//! carry rule ids and error codes, never content.

use std::collections::VecDeque;

use copypaste_core::rules::apply::{CaptureEffects, Original};
use copypaste_core::rules::evaluate::{
    ContentTypes, EvalFlags, EvaluationContext, EvaluationInput, PrivacyMarkers, TransformStep,
};
use copypaste_core::rules::model::{CaptureOrigin, PrivacyMarker};
use copypaste_core::storage::organization::{Record, RecordKey};
use copypaste_core::PayloadMetadata;
use copypaste_ipc::rules::SkipEvent;
use copypaste_ipc::ClipboardPrivacy;
use copypaste_transform::Step;

use crate::{locked, Runtime};

/// Skipped captures kept in memory for "Recently skipped".
pub const MAX_SKIPPED_CAPTURES: usize = 50;

/// What content rules read about one capture.
#[derive(Debug, Clone, Copy)]
pub struct CaptureSubject<'a> {
    /// The text conditions read: a text capture's content, a file's name and
    /// locator, nothing for an image.
    pub text: Option<&'a str>,
    pub content_type: &'a str,
    pub byte_len: u64,
    pub source_app: Option<&'a str>,
    pub source_app_name: Option<&'a str>,
    /// The producer's privacy markers.
    pub privacy: ClipboardPrivacy,
    pub created_at: i64,
}

#[derive(Debug)]
pub enum CaptureDecision {
    /// A rule said "Don't save": nothing may be stored.
    Skip,
    Store(CapturePlan),
}

/// How a capture is stored after its rules ran.
#[derive(Debug, Default)]
pub struct CapturePlan {
    secret: bool,
    text: Option<String>,
    replace_clipboard: bool,
    effects: CaptureEffects,
}

impl CapturePlan {
    /// Protection in place of rules that could not be read (R-20).
    fn fail_closed() -> Self {
        Self {
            secret: true,
            effects: CaptureEffects {
                hide_from_integrations: true,
                flags: EvalFlags::RULES_INCOMPLETE,
                ..CaptureEffects::default()
            },
            ..Self::default()
        }
    }

    /// The text to store: the transformed result, or what was captured.
    #[must_use]
    pub fn text<'a>(&'a self, captured: &'a str) -> &'a str {
        self.text.as_deref().unwrap_or(captured)
    }

    #[must_use]
    pub fn secret(&self) -> bool {
        self.secret
    }

    /// `metadata` with the secret a rule set.
    #[must_use]
    pub fn metadata(&self, metadata: Option<PayloadMetadata>) -> Option<PayloadMetadata> {
        if !self.secret {
            return metadata;
        }
        let mut metadata = metadata.unwrap_or_default();
        metadata.privacy.secret = true;
        Some(metadata)
    }

    /// Effects the core writes in the transaction that stores the capture.
    #[must_use]
    pub fn effects(&self) -> &CaptureEffects {
        &self.effects
    }
}

impl Runtime {
    /// Evaluates this device's content rules on a capture before it is
    /// stored. A skip is logged here; the caller stores nothing.
    pub fn evaluate_capture(&self, subject: &CaptureSubject<'_>) -> CaptureDecision {
        let rules = match self.store.content_rules() {
            Ok(rules) => rules,
            Err(error) => {
                tracing::warn!(
                    ?error,
                    "content rules unavailable; the capture is protected"
                );
                return CaptureDecision::Store(CapturePlan::fail_closed());
            }
        };
        if rules.set.is_empty() {
            return CaptureDecision::Store(CapturePlan::default());
        }
        let markers = PrivacyMarkers::UNKNOWN
            .with(PrivacyMarker::Concealed, subject.privacy.secret)
            .with(PrivacyMarker::Transient, subject.privacy.transient);
        let outcome = rules.set.evaluate(
            &EvaluationInput {
                text: subject.text,
                content_types: ContentTypes::from_content_type(subject.content_type),
                byte_len: subject.byte_len,
                source_app: subject.source_app,
                markers,
                origin: CaptureOrigin::LocalCopy,
                created_at: subject.created_at,
            },
            EvaluationContext::Capture,
        );
        if let Some(skip) = &outcome.dont_save {
            self.record_skip(subject, &skip.rule_id);
            // The skipped copy is still on the clipboard: a pending restore
            // must not overwrite it, only the rule's own clear may.
            match skip.clear_clipboard_after_secs {
                Some(seconds) => {
                    self.schedule_clipboard_clear(i64::from(seconds).saturating_mul(1000));
                }
                None => self.note_clipboard_changed(),
            }
            return CaptureDecision::Skip;
        }
        let mut plan = CapturePlan {
            secret: outcome.secret,
            effects: CaptureEffects::from_outcome(&outcome, rules.revision),
            ..CapturePlan::default()
        };
        let text = subject
            .text
            .filter(|_| copypaste_ipc::content_type::is_text(subject.content_type));
        if let Some(text) = text.filter(|_| !plan.secret && !subject.privacy.secret) {
            self.transform_capture(&mut plan, &outcome.transforms, text, subject.content_type);
        }
        CaptureDecision::Store(plan)
    }

    /// Follows a stored capture: arms its expiry, and puts a transformed
    /// result on the clipboard as an own write, which is never captured again.
    pub fn finish_capture(&self, item_id: &str, plan: &CapturePlan) {
        if plan.effects.expiry.is_some() {
            match self.store.clip_local_state(item_id) {
                Ok(state) => {
                    if let Some(at) = state.and_then(|state| state.expires_at) {
                        self.note_store_deadline(at);
                    }
                }
                Err(error) => tracing::warn!(?error, "a capture's expiry could not be read"),
            }
        }
        if plan.replace_clipboard {
            if let Err(error) = self.deliver_item(item_id, false, false) {
                tracing::warn!(
                    ?error,
                    "a transformed capture could not replace the clipboard"
                );
            }
        }
    }

    /// Captures content rules skipped, oldest first. Never persisted.
    pub fn skipped_captures(&self) -> Vec<SkipEvent> {
        locked(&self.skipped_captures).iter().cloned().collect()
    }

    /// Runs the rules' transforms in order. Secrets, credentials and results
    /// that could not be stored keep the captured text (R-8).
    fn transform_capture(
        &self,
        plan: &mut CapturePlan,
        steps: &[TransformStep],
        text: &str,
        content_type: &str,
    ) {
        if steps.is_empty()
            || copypaste_core::classify_credentials(content_type, text)
                .is_some_and(|privacy| privacy.secret)
        {
            return;
        }
        let parsed: Option<Vec<Step>> = steps
            .iter()
            .map(|rule| {
                Step::current(&rule.transform_id).map(|step| Step {
                    params: rule.params.clone(),
                    ..step
                })
            })
            .collect();
        let Some(parsed) = parsed else {
            tracing::warn!("a content rule names an unknown transform; storing the capture as is");
            return;
        };
        let limit = self.settings.get().capture_limit_bytes(content_type);
        match self.run_capture_transforms(parsed, text) {
            Ok((output, _)) if output == text || output.trim().is_empty() => {}
            Ok((output, _)) if output.len() as u64 > limit => {
                tracing::warn!("a content rule transform grew the capture past its limit");
            }
            Ok((output, sensitive)) => {
                plan.secret |= sensitive;
                if !plan.secret {
                    plan.effects.original = Some(Original::new(text.as_bytes(), content_type));
                }
                plan.text = Some(output);
                plan.replace_clipboard = steps.iter().any(|step| step.replace_clipboard);
            }
            Err(error) => tracing::warn!(
                code = error.code(),
                "a content rule transform failed; storing the capture as is"
            ),
        }
    }

    fn record_skip(&self, subject: &CaptureSubject<'_>, rule_id: &str) {
        let rule_name = match self
            .store
            .organization_record(RecordKey::ContentRule(rule_id))
        {
            Ok(Some(Record::ContentRule(rule))) => rule.name,
            _ => rule_id.to_owned(),
        };
        tracing::debug!(rule = rule_id, "a content rule skipped a capture");
        let mut skipped = locked(&self.skipped_captures);
        if skipped.len() >= MAX_SKIPPED_CAPTURES {
            skipped.pop_front();
        }
        skipped.push_back(SkipEvent {
            at: subject.created_at,
            source_app_name: subject
                .source_app_name
                .or(subject.source_app)
                .map(str::to_owned),
            rule_name,
        });
    }
}

/// The in-memory "Recently skipped" ring.
pub(crate) type SkippedCaptures = std::sync::Mutex<VecDeque<SkipEvent>>;

#[cfg(test)]
mod tests {
    use copypaste_core::storage::organization::{
        ContentRuleRecord, Record, RuleOrigin, RulePlacementRecord,
    };

    use crate::tests::fixture;
    use crate::{Runtime, RuntimeError};

    fn add_rule(runtime: &Runtime, id: &str, definition: &str) {
        let rule = ContentRuleRecord {
            id: id.into(),
            origin: RuleOrigin::User,
            builtin_version: None,
            customized_fields: Vec::new(),
            name: format!("Rule {id}"),
            definition_json: definition.into(),
            device_scope: Vec::new(),
            schema_version: 1,
            effective_from: 0,
            deleted_at: None,
            base_stamp: None,
        };
        let placement = RulePlacementRecord {
            id: id.into(),
            enabled: true,
            order_key: id.into(),
        };
        runtime
            .store
            .write_organization_records(
                &runtime.keyring.item_key(),
                [
                    Record::ContentRule(rule),
                    Record::ContentRulePlacement(placement),
                ],
            )
            .unwrap();
    }

    #[test]
    fn in_process_captures_run_rules_before_anything_is_stored() {
        let (runtime, _dir) = fixture();
        add_rule(
            &runtime,
            "a",
            r#"{"match":"all","conditions":[{"type":"content_text","op":"contains","value":"skip me"}],"actions":[{"type":"dont_save"}]}"#,
        );
        add_rule(
            &runtime,
            "b",
            r#"{"match":"all","conditions":[{"type":"content_text","op":"contains","value":"shout"}],"actions":[{"type":"transform","transform_id":"case.upper"},{"type":"hide_from_integrations"}]}"#,
        );
        assert!(matches!(
            runtime.capture_text("please skip me"),
            Err(RuntimeError::CaptureRefused)
        ));
        assert_eq!(runtime.store.count().unwrap(), 0);
        assert_eq!(runtime.skipped_captures()[0].rule_name, "Rule a");

        runtime.capture_text("shout it").unwrap();
        let row = runtime.store.list(1, 0).unwrap().remove(0);
        let annotations = runtime
            .store
            .item_annotations(std::slice::from_ref(&row.id))
            .unwrap();
        assert!(annotations[&row.id].hidden_from_integrations);
        let text = copypaste_core::decrypt(
            &row.content_ciphertext,
            &row.nonce,
            &runtime.keyring.item_key(),
            &row.id,
        )
        .unwrap();
        assert_eq!(text.as_slice(), b"SHOUT IT");
    }
}
