#![cfg_attr(not(any(target_os = "android", test)), allow(dead_code))]

use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;

pub(crate) type LifecycleFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub(crate) trait LifecycleOperation {
    fn acquire(&self) -> LifecycleFuture<'_, bool>;
    fn advertise<'a>(
        &'a self,
        name: &'a str,
        pairing_ids: &'a [String],
    ) -> LifecycleFuture<'a, bool>;
    fn release(&self) -> LifecycleFuture<'_, ()>;
}

#[derive(Clone, Default)]
pub(crate) struct Desired {
    pub(crate) name: String,
    pub(crate) pairing_ids: Vec<String>,
    pub(crate) visible: bool,
}

#[derive(Default)]
struct Lifecycle {
    generation: u64,
    desired: Desired,
    running: bool,
}

#[derive(Default)]
pub(crate) struct LifecycleCoordinator {
    lifecycle: Mutex<Lifecycle>,
}

impl LifecycleCoordinator {
    pub(crate) fn reconcile(&self, desired: Desired) -> bool {
        let mut lifecycle = self
            .lifecycle
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        lifecycle.generation = lifecycle.generation.saturating_add(1);
        lifecycle.desired = desired;
        if lifecycle.running {
            return false;
        }
        lifecycle.running = true;
        true
    }

    fn current(&self) -> (u64, Desired) {
        let lifecycle = self
            .lifecycle
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        (lifecycle.generation, lifecycle.desired.clone())
    }

    fn current_generation(&self, generation: u64) -> bool {
        self.lifecycle
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .generation
            == generation
    }

    fn finish(&self, generation: u64) -> bool {
        let mut lifecycle = self
            .lifecycle
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if lifecycle.generation != generation {
            return false;
        }
        lifecycle.running = false;
        true
    }

    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub(crate) fn stop(&self) {
        self.lifecycle
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .running = false;
    }

    #[cfg(test)]
    fn desired(&self) -> Desired {
        self.lifecycle
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .desired
            .clone()
    }

    #[cfg(test)]
    fn is_running(&self) -> bool {
        self.lifecycle
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .running
    }

    pub(crate) async fn drain<O: LifecycleOperation>(&self, operation: &O) {
        loop {
            let (generation, desired) = self.current();
            if !desired.visible {
                operation.release().await;
            } else if operation.acquire().await
                && self.current_generation(generation)
                && !operation
                    .advertise(&desired.name, &desired.pairing_ids)
                    .await
            {
                tracing::warn!("Android LAN discovery did not start advertising");
            }
            if self.finish(generation) {
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::Arc;

    use tokio::sync::oneshot;

    use super::{Desired, LifecycleCoordinator, LifecycleFuture, LifecycleOperation};
    use std::sync::Mutex;

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum LifecycleEvent {
        Acquire,
        Advertise(String),
        Release,
    }

    enum AcquireResult {
        Immediate(bool),
        Delayed {
            started: oneshot::Sender<()>,
            result: oneshot::Receiver<bool>,
        },
    }

    struct ControlledOperation {
        acquisitions: Mutex<VecDeque<AcquireResult>>,
        events: Mutex<Vec<LifecycleEvent>>,
    }

    impl ControlledOperation {
        fn new(acquisitions: impl IntoIterator<Item = AcquireResult>) -> Self {
            Self {
                acquisitions: Mutex::new(acquisitions.into_iter().collect()),
                events: Mutex::new(Vec::new()),
            }
        }

        fn events(&self) -> Vec<LifecycleEvent> {
            self.events
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone()
        }
    }

    impl LifecycleOperation for ControlledOperation {
        fn acquire(&self) -> LifecycleFuture<'_, bool> {
            let acquisition = self
                .acquisitions
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .pop_front()
                .unwrap_or(AcquireResult::Immediate(false));
            let events = &self.events;
            Box::pin(async move {
                events
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .push(LifecycleEvent::Acquire);
                match acquisition {
                    AcquireResult::Immediate(available) => available,
                    AcquireResult::Delayed { started, result } => {
                        let _ = started.send(());
                        result.await.unwrap_or(false)
                    }
                }
            })
        }

        fn advertise<'a>(
            &'a self,
            name: &'a str,
            _pairing_ids: &'a [String],
        ) -> LifecycleFuture<'a, bool> {
            let events = &self.events;
            let name = name.to_owned();
            Box::pin(async move {
                events
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .push(LifecycleEvent::Advertise(name));
                true
            })
        }

        fn release(&self) -> LifecycleFuture<'_, ()> {
            let events = &self.events;
            Box::pin(async move {
                events
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .push(LifecycleEvent::Release);
            })
        }
    }

    fn desired(name: &str, visible: bool) -> Desired {
        Desired {
            name: name.into(),
            pairing_ids: Vec::new(),
            visible,
        }
    }

    #[tokio::test]
    async fn visibility_loss_during_acquire_releases_without_advertising() {
        let (acquire_started, acquire_started_rx) = oneshot::channel();
        let (acquire_result, acquire_result_rx) = oneshot::channel();
        let lifecycle = Arc::new(LifecycleCoordinator::default());
        let operation = Arc::new(ControlledOperation::new([AcquireResult::Delayed {
            started: acquire_started,
            result: acquire_result_rx,
        }]));

        assert!(lifecycle.reconcile(desired("Visible", true)));
        let drain = {
            let lifecycle = Arc::clone(&lifecycle);
            let operation = Arc::clone(&operation);
            tokio::spawn(async move { lifecycle.drain(operation.as_ref()).await })
        };
        acquire_started_rx.await.unwrap();
        assert!(!lifecycle.reconcile(desired("Hidden", false)));
        acquire_result.send(true).unwrap();
        drain.await.unwrap();

        assert_eq!(
            operation.events(),
            vec![LifecycleEvent::Acquire, LifecycleEvent::Release],
        );
    }

    #[tokio::test]
    async fn rapid_reconciles_advertise_only_the_latest_name() {
        let (acquire_started, acquire_started_rx) = oneshot::channel();
        let (acquire_result, acquire_result_rx) = oneshot::channel();
        let lifecycle = Arc::new(LifecycleCoordinator::default());
        let operation = Arc::new(ControlledOperation::new([
            AcquireResult::Delayed {
                started: acquire_started,
                result: acquire_result_rx,
            },
            AcquireResult::Immediate(true),
        ]));

        assert!(lifecycle.reconcile(desired("First", true)));
        let drain = {
            let lifecycle = Arc::clone(&lifecycle);
            let operation = Arc::clone(&operation);
            tokio::spawn(async move { lifecycle.drain(operation.as_ref()).await })
        };
        acquire_started_rx.await.unwrap();
        assert!(!lifecycle.reconcile(desired("Second", true)));
        assert!(!lifecycle.reconcile(desired("Latest", true)));
        acquire_result.send(true).unwrap();
        drain.await.unwrap();

        assert_eq!(
            operation.events(),
            vec![
                LifecycleEvent::Acquire,
                LifecycleEvent::Acquire,
                LifecycleEvent::Advertise("Latest".into()),
            ],
        );
    }

    #[test]
    fn reading_desired_state_does_not_start_lifecycle_work() {
        let lifecycle = LifecycleCoordinator::default();

        let desired = lifecycle.desired();

        assert_eq!(desired.name, "");
        assert!(!desired.visible);
        assert!(!lifecycle.is_running());
    }
}
