//! Bounded foreground evidence, independent of Cocoa and payload access.

use std::collections::{BTreeSet, VecDeque};

pub(crate) const MAX_ACTIVATIONS: usize = 128;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SourceIdentity {
    pub(crate) bundle_id: Option<String>,
    pub(crate) name: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Observation {
    pub(crate) service_id: u64,
    pub(crate) epoch: u64,
    real_epoch: u64,
    active_known: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Incomplete {
    MissingBoundary,
    ServiceChanged,
    NonmonotonicGeneration,
    Evicted,
    Unknown,
    Exhausted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SourceConfidence {
    SingleObservedApplication,
    AmbiguousObservedApplications,
    Unavailable,
}

/// This witness is only produced by the foreground coverage reducer. Missing
/// display metadata is not the same thing as missing admission evidence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Coverage {
    start: Option<Observation>,
    end: Observation,
    incomplete: Option<Incomplete>,
    candidates: BTreeSet<String>,
}

impl Coverage {
    pub(crate) fn allows(&self, excluded: &[String]) -> bool {
        excluded.is_empty()
            || (self.incomplete.is_none()
                && !self.candidates.is_empty()
                && self.candidates.iter().all(|id| !excluded.contains(id)))
    }

    pub(crate) fn confidence(&self) -> SourceConfidence {
        if self.incomplete.is_some() || self.candidates.is_empty() {
            SourceConfidence::Unavailable
        } else if self.candidates.len() == 1 {
            SourceConfidence::SingleObservedApplication
        } else {
            SourceConfidence::AmbiguousObservedApplications
        }
    }

    fn single_candidate(&self) -> Option<&str> {
        (self.incomplete.is_none() && self.candidates.len() == 1)
            .then(|| self.candidates.first().unwrap().as_str())
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Decision {
    pub(crate) generation: i64,
    pub(crate) sample: Observation,
    pub(crate) coverage: Coverage,
    pub(crate) identity: Option<SourceIdentity>,
    recovery: Option<Observation>,
}

impl Decision {
    pub(crate) fn unavailable(generation: i64, sample: Option<Observation>) -> Self {
        let sample = sample.unwrap_or(Observation {
            service_id: 0,
            epoch: 0,
            real_epoch: 0,
            active_known: false,
        });
        Self {
            generation,
            sample,
            coverage: Coverage {
                start: None,
                end: sample,
                incomplete: Some(Incomplete::MissingBoundary),
                candidates: BTreeSet::new(),
            },
            identity: None,
            recovery: None,
        }
    }

    /// The production read envelope performs no payload calls for a denied
    /// interval and never returns a payload invalidated while materializing.
    pub(crate) fn materialize<T>(
        &self,
        excluded: &[String],
        mut observe: impl FnMut() -> (i64, Option<Observation>),
        read: impl FnOnce() -> Option<T>,
    ) -> Option<T> {
        let (generation, observation) = observe();
        if !self.coverage.allows(excluded) || !self.fence(observation, generation) {
            return None;
        }
        let payload = read()?;
        let (generation, observation) = observe();
        self.fence(observation, generation).then_some(payload)
    }

    pub(crate) fn fence(&self, current: Option<Observation>, generation: i64) -> bool {
        generation == self.generation
            && match current {
                Some(current) => current == self.coverage.end,
                None => self.coverage.end.service_id == 0,
            }
    }
}

struct Activation {
    epoch: u64,
    bundle_id: Option<String>,
}

pub(crate) struct ActivationHistory {
    service_id: u64,
    epoch: u64,
    real_epoch: u64,
    events: VecDeque<Activation>,
    active: Option<String>,
}

impl ActivationHistory {
    pub(crate) fn new(service_id: u64) -> Self {
        Self {
            service_id,
            epoch: 0,
            real_epoch: 0,
            events: VecDeque::new(),
            active: None,
        }
    }

    pub(crate) fn observation(&self) -> Observation {
        Observation {
            service_id: self.service_id,
            epoch: self.epoch,
            real_epoch: self.real_epoch,
            active_known: self.active.is_some(),
        }
    }

    pub(crate) fn active_is_known(&self) -> bool {
        self.active.is_some()
    }

    fn append(&mut self, bundle_id: Option<String>) -> u64 {
        self.active = bundle_id.clone();
        if let Some(epoch) = self.epoch.checked_add(1) {
            self.epoch = epoch;
            self.events.push_back(Activation { epoch, bundle_id });
            if self.events.len() > MAX_ACTIVATIONS {
                self.events.pop_front();
            }
        } else {
            self.events.clear();
            self.active = None;
        }
        self.epoch
    }

    pub(crate) fn record(&mut self, bundle_id: Option<String>) -> u64 {
        let epoch = self.append(bundle_id);
        self.real_epoch = epoch;
        epoch
    }

    pub(crate) fn deactivate(&mut self, bundle_id: Option<String>) -> u64 {
        let epoch = self.record(bundle_id);
        self.active = None;
        epoch
    }

    pub(crate) fn reconcile(&mut self, bundle_id: Option<&str>) -> u64 {
        if self.active.as_deref() != bundle_id || self.events.is_empty() {
            // Getter disagreement contributes gap debt to the old interval.
            // It is not a notification, so a consumed Unknown can separately
            // recover at the reconciled boundary without skipping real events.
            self.append(None);
            self.append(bundle_id.map(str::to_owned));
        }
        self.epoch
    }

    pub(crate) fn decide(
        &self,
        previous: Option<(i64, Observation)>,
        generation: i64,
        sample: Observation,
        identity: Option<SourceIdentity>,
    ) -> Decision {
        let end = self.observation();
        let mut coverage = Coverage {
            start: previous.map(|(_, cursor)| cursor),
            end,
            incomplete: None,
            candidates: BTreeSet::new(),
        };
        coverage.incomplete = if self.service_id == 0 || self.epoch == u64::MAX {
            Some(Incomplete::Exhausted)
        } else if sample.service_id != self.service_id {
            Some(Incomplete::ServiceChanged)
        } else if let Some((old_generation, start)) = previous {
            if start.service_id != self.service_id {
                Some(Incomplete::ServiceChanged)
            } else if generation <= old_generation {
                Some(Incomplete::NonmonotonicGeneration)
            } else if let Some(index) = self
                .events
                .iter()
                .position(|event| event.epoch == start.epoch)
            {
                for event in self.events.iter().skip(index) {
                    if let Some(bundle) = &event.bundle_id {
                        coverage.candidates.insert(bundle.clone());
                    } else {
                        coverage.incomplete = Some(Incomplete::Unknown);
                    }
                }
                if !end.active_known {
                    coverage.incomplete = Some(Incomplete::Unknown);
                }
                coverage.incomplete
            } else {
                Some(Incomplete::Evicted)
            }
        } else {
            Some(Incomplete::MissingBoundary)
        };
        let identity = identity.filter(|identity| {
            coverage
                .single_candidate()
                .is_some_and(|id| identity.bundle_id.as_deref() == Some(id))
        });
        let recovery = (!sample.active_known
            && end.active_known
            && sample.service_id == end.service_id
            && sample.real_epoch == end.real_epoch
            && self.epoch != u64::MAX
            && self.service_id != 0)
            .then_some(end);
        Decision {
            generation,
            sample,
            coverage,
            identity,
            recovery,
        }
    }
}

/// The acknowledged generation owns its next interval. Idle samples retain the
/// boundary; only an explicitly consumed Unknown may recover at the same count.
#[derive(Default)]
pub(crate) struct GenerationCoverage {
    boundary: Option<(i64, Observation)>,
    consumed_generation: Option<i64>,
}

impl GenerationCoverage {
    pub(crate) fn boundary(&self) -> Option<(i64, Observation)> {
        self.boundary
    }

    pub(crate) fn unchanged(&mut self, generation: i64, observation: Option<Observation>) {
        if self.consumed_generation != Some(generation) {
            return;
        }
        let Some(current) = observation else {
            return;
        };
        if current.service_id == 0 || current.epoch == u64::MAX || !current.active_known {
            return;
        }
        match self.boundary {
            Some((_, old))
                if !old.active_known
                    && old.service_id == current.service_id
                    && old.real_epoch == current.real_epoch =>
            {
                self.boundary = Some((generation, current))
            }
            _ => {}
        }
    }

    pub(crate) fn consume(
        &mut self,
        generation: i64,
        sample: Option<Observation>,
        decision: Option<&Decision>,
        same_count: bool,
    ) {
        self.consumed_generation = Some(generation);
        let recovery = same_count
            .then(|| {
                decision
                    .filter(|decision| {
                        decision.generation == generation && Some(decision.sample) == sample
                    })
                    .and_then(|decision| decision.recovery)
            })
            .flatten();
        self.boundary = recovery.or(sample).map(|cursor| (generation, cursor));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn app(id: &str) -> Option<String> {
        Some(id.into())
    }
    fn excluded() -> Vec<String> {
        vec!["Safari".into()]
    }
    fn decide(
        history: &ActivationHistory,
        owner: &GenerationCoverage,
        generation: i64,
    ) -> Decision {
        let sample = history.observation();
        history.decide(
            owner.boundary(),
            generation,
            sample,
            Some(SourceIdentity {
                bundle_id: history.active.clone(),
                name: history.active.clone(),
            }),
        )
    }
    fn baseline(history: &ActivationHistory) -> GenerationCoverage {
        let mut owner = GenerationCoverage::default();
        owner.consume(10, Some(history.observation()), None, true);
        owner
    }

    #[test]
    fn idle_samples_cannot_erase_excluded_or_unknown_debt() {
        for gap in [false, true] {
            let mut history = ActivationHistory::new(1);
            history.record(app("TextEdit"));
            let mut owner = baseline(&history);
            history.record(if gap { None } else { app("Safari") });
            history.record(app("TextEdit"));
            for _ in 0..5 {
                owner.unchanged(10, Some(history.observation()));
            }
            let denied = decide(&history, &owner, 11);
            assert!(!denied.coverage.allows(&excluded()));
            owner.consume(11, Some(denied.sample), Some(&denied), true);
            assert!(decide(&history, &owner, 12).coverage.allows(&excluded()));
        }
    }
    #[test]
    fn multiple_allowed_candidates_admit_without_guessed_identity() {
        let mut history = ActivationHistory::new(1);
        history.record(app("TextEdit"));
        let mut owner = baseline(&history);
        history.record(app("CopyPaste"));
        let decision = decide(&history, &owner, 11);
        assert!(decision.coverage.allows(&excluded()));
        assert!(decision.identity.is_none());
        assert!(!decision.coverage.allows(&vec!["TextEdit".into()]));
        owner.consume(11, Some(decision.sample), Some(&decision), true);
        assert_eq!(
            decide(&history, &owner, 12)
                .identity
                .unwrap()
                .bundle_id
                .as_deref(),
            Some("CopyPaste")
        );
    }
    #[test]
    fn consumed_unknown_recovers_without_skipping_later_real_events() {
        for later_event in [false, true] {
            let mut history = ActivationHistory::new(1);
            history.record(None);
            let mut owner = baseline(&history);
            let sample = history.observation();
            if later_event {
                history.record(app("Safari"));
                history.record(None);
            }
            history.reconcile(Some("TextEdit"));
            let denied = history.decide(owner.boundary(), 11, sample, None);
            assert!(!denied.coverage.allows(&excluded()));
            owner.consume(11, Some(sample), Some(&denied), true);
            owner.unchanged(11, Some(history.observation()));
            assert_eq!(
                decide(&history, &owner, 12).coverage.allows(&excluded()),
                !later_event
            );
        }
    }
    #[test]
    fn events_after_sample_survive_consumption_and_invalidate_read() {
        let mut history = ActivationHistory::new(1);
        history.record(app("TextEdit"));
        let mut owner = baseline(&history);
        let decision = decide(&history, &owner, 11);
        history.record(app("Safari"));
        assert!(!decision.fence(Some(history.observation()), 11));
        owner.consume(11, Some(decision.sample), Some(&decision), true);
        assert!(!decide(&history, &owner, 12).coverage.allows(&excluded()));
        assert!(!decision.fence(Some(decision.coverage.end), 12));
    }
    #[test]
    fn excluded_active_and_visit_without_copy_drop_once_then_recover() {
        let mut history = ActivationHistory::new(1);
        history.record(app("Safari"));
        let mut owner = baseline(&history);
        history.deactivate(app("Safari"));
        history.record(app("TextEdit"));
        owner.unchanged(10, Some(history.observation()));
        let denied = decide(&history, &owner, 11);
        assert!(!denied.coverage.allows(&excluded()));
        owner.consume(11, Some(denied.sample), Some(&denied), true);
        assert!(decide(&history, &owner, 12).coverage.allows(&excluded()));
    }
    #[test]
    fn missing_evicted_replaced_nonmonotonic_and_exhausted_fail_closed() {
        let mut history = ActivationHistory::new(1);
        history.record(app("TextEdit"));
        assert!(!decide(&history, &GenerationCoverage::default(), 11)
            .coverage
            .allows(&excluded()));
        let owner = baseline(&history);
        assert!(!decide(&history, &owner, 10).coverage.allows(&excluded()));
        let mut other = ActivationHistory::new(2);
        other.record(app("TextEdit"));
        assert!(!decide(&other, &owner, 11).coverage.allows(&excluded()));
        for _ in 0..MAX_ACTIVATIONS {
            history.record(app("TextEdit"));
        }
        assert!(!decide(&history, &owner, 11).coverage.allows(&excluded()));
        history.epoch = u64::MAX;
        assert!(!decide(&history, &baseline(&history), 11)
            .coverage
            .allows(&excluded()));
    }
    #[test]
    fn actual_change_tracker_idle_sequence_denies_before_payload_calls() {
        use crate::clipboard::change::{Change, ChangeTracker};
        let mut history = ActivationHistory::new(1);
        history.record(app("TextEdit"));
        let mut owner = GenerationCoverage::default();
        let mut tracker = ChangeTracker::new();
        assert!(matches!(tracker.observe(10), Change::Fresh { .. }));
        owner.consume(10, Some(history.observation()), None, true);
        history.record(app("Safari"));
        history.record(app("TextEdit"));
        for _ in 0..4 {
            assert!(tracker.is_current(10)); // changed() path
            owner.unchanged(10, Some(history.observation()));
            assert_eq!(tracker.observe(10), Change::Unchanged); // poll() path
            owner.unchanged(10, Some(history.observation()));
        }
        assert!(matches!(tracker.observe(11), Change::Fresh { .. }));
        let decision = decide(&history, &owner, 11);
        let calls = std::cell::Cell::new(0);
        let payload = decision.materialize(
            &excluded(),
            || (11, Some(history.observation())),
            || {
                calls.set(calls.get() + 1);
                Some(vec![1, 2, 3])
            },
        );
        assert!(payload.is_none());
        assert_eq!(calls.get(), 0);
        owner.consume(11, Some(decision.sample), Some(&decision), true);
        let recovered = decide(&history, &owner, 12);
        assert!(recovered
            .materialize(
                &excluded(),
                || (12, Some(history.observation())),
                || {
                    calls.set(calls.get() + 1);
                    Some(vec![4])
                }
            )
            .is_some());
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn materialization_replacement_before_read_and_before_freeze_never_returns_payload() {
        let mut history = ActivationHistory::new(1);
        history.record(app("TextEdit"));
        let owner = baseline(&history);
        let decision = decide(&history, &owner, 11);
        let calls = std::cell::Cell::new(0);
        assert!(decision
            .materialize(
                &[],
                || (12, Some(history.observation())),
                || {
                    calls.set(calls.get() + 1);
                    Some(vec![1])
                }
            )
            .is_none());
        assert_eq!(calls.get(), 0);
        let generation = std::cell::Cell::new(11);
        assert!(decision
            .materialize(
                &[],
                || (generation.get(), Some(history.observation())),
                || {
                    calls.set(calls.get() + 1);
                    generation.set(12);
                    Some(vec![1])
                }
            )
            .is_none());
        assert_eq!(calls.get(), 1);
        let observation = std::cell::Cell::new(history.observation());
        assert!(decision
            .materialize(
                &[],
                || (11, Some(observation.get())),
                || {
                    let mut changed = observation.get();
                    changed.epoch += 1;
                    observation.set(changed);
                    Some(vec![1])
                }
            )
            .is_none());
    }

    #[test]
    fn same_count_recovery_cannot_clear_unconsumed_gap_or_excluded_debt() {
        let mut history = ActivationHistory::new(1);
        history.record(app("TextEdit"));
        let mut owner = baseline(&history);
        history.record(None);
        history.reconcile(Some("TextEdit"));
        owner.unchanged(10, Some(history.observation()));
        assert!(!decide(&history, &owner, 11).coverage.allows(&excluded()));
        let unknown = history.observation();
        history.record(app("Safari"));
        let denied = history.decide(owner.boundary(), 11, unknown, None);
        owner.consume(11, Some(unknown), Some(&denied), true);
        assert!(!decide(&history, &owner, 12).coverage.allows(&excluded()));
    }
    #[test]
    fn startup_and_service_replacement_recover_only_after_acknowledgement() {
        let mut history = ActivationHistory::new(1);
        let mut owner = GenerationCoverage::default();
        let startup = decide(&history, &owner, 10);
        assert!(!startup.coverage.allows(&excluded()));
        owner.consume(10, Some(startup.sample), Some(&startup), true);
        history.reconcile(Some("TextEdit"));
        owner.unchanged(10, Some(history.observation()));
        assert!(decide(&history, &owner, 11).coverage.allows(&excluded()));
        let mut replaced = ActivationHistory::new(2);
        replaced.record(app("TextEdit"));
        let denied = decide(&replaced, &owner, 11);
        assert!(!denied.coverage.allows(&excluded()));
        owner.consume(11, Some(denied.sample), Some(&denied), true);
        assert!(decide(&replaced, &owner, 12).coverage.allows(&excluded()));
    }

    #[test]
    fn self_write_consumption_does_not_replay_or_poison_a_later_clean_generation() {
        use crate::clipboard::change::{Change, ChangeTracker};
        let mut history = ActivationHistory::new(1);
        history.record(app("TextEdit"));
        let mut owner = baseline(&history);
        let mut tracker = ChangeTracker::new();
        tracker.observe(10);
        tracker.sentinel.arm(11);
        assert_eq!(tracker.observe(11), Change::SelfWrite);
        owner.consume(11, Some(history.observation()), None, true);
        assert_eq!(tracker.observe(11), Change::Unchanged);
        assert!(matches!(tracker.observe(12), Change::Fresh { .. }));
        assert!(decide(&history, &owner, 12).coverage.allows(&excluded()));
    }

    #[test]
    fn missing_consumed_boundary_keeps_later_excluded_activity_until_fresh_consumption() {
        let mut owner = GenerationCoverage::default();
        owner.consume(10, None, None, true);
        let mut history = ActivationHistory::new(1);
        history.record(app("Safari"));
        history.record(app("TextEdit"));
        for _ in 0..5 {
            owner.unchanged(10, Some(history.observation()));
        }
        let denied = decide(&history, &owner, 11);
        let payload_calls = std::cell::Cell::new(0);
        assert!(!denied.coverage.allows(&excluded()));
        assert!(denied
            .materialize(
                &excluded(),
                || (11, Some(history.observation())),
                || {
                    payload_calls.set(payload_calls.get() + 1);
                    Some(vec![1, 2, 3])
                }
            )
            .is_none());
        assert_eq!(payload_calls.get(), 0);

        owner.consume(11, Some(denied.sample), Some(&denied), true);
        let recovered = decide(&history, &owner, 12);
        assert!(recovered
            .materialize(
                &excluded(),
                || (12, Some(history.observation())),
                || {
                    payload_calls.set(payload_calls.get() + 1);
                    Some(vec![4, 5, 6])
                }
            )
            .is_some());
        assert_eq!(payload_calls.get(), 1);
    }
}
