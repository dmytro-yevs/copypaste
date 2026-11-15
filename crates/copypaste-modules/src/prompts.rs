//! Questions modules ask the user. The asking module thread waits on its own
//! bounded channel; nothing is polled, and an unanswered prompt resolves to
//! dismissed at its deadline, when it is answered, or when its module stops.
use crate::ModuleError;
use copypaste_module_sdk::{
    HostError, HostErrorCode, ModulePrompt, PromptAnswer, PromptRequest, MAX_PENDING_PROMPTS,
};
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// Called with the number of pending prompts after every change.
pub type PromptListener = Arc<dyn Fn(u32) + Send + Sync>;

#[derive(Default)]
pub(crate) struct Prompts {
    state: Mutex<State>,
    listener: Mutex<Option<PromptListener>>,
}

#[derive(Default)]
struct State {
    next: u64,
    pending: BTreeMap<String, Pending>,
}

struct Pending {
    owner: u64,
    prompt: ModulePrompt,
    answer: mpsc::SyncSender<Option<String>>,
}

impl Prompts {
    pub(crate) fn set_listener(&self, listener: PromptListener) {
        *self.listener.lock().unwrap_or_else(|e| e.into_inner()) = Some(listener);
    }

    /// Blocks the calling module thread until the prompt resolves. `owner`
    /// names the module instance, refused once `accepting` is cleared.
    pub(crate) fn ask(
        &self,
        module_id: &str,
        owner: u64,
        accepting: &AtomicBool,
        request: PromptRequest,
    ) -> Result<PromptAnswer, HostError> {
        let timeout = Duration::from_secs(u64::from(request.timeout_secs));
        let expires_at_ms = SystemTime::now()
            .checked_add(timeout)
            .and_then(|deadline| deadline.duration_since(UNIX_EPOCH).ok())
            .map_or(i64::MAX, |deadline| {
                i64::try_from(deadline.as_millis()).unwrap_or(i64::MAX)
            });
        let (answer, received) = mpsc::sync_channel(1);
        let prompt_id = {
            let mut state = self.lock();
            // Checked under the lock `dismiss` takes, so a stopping instance
            // cannot leave a prompt behind.
            if !accepting.load(Ordering::Acquire) {
                return Err(HostError::new(
                    HostErrorCode::Unavailable,
                    "The module is stopping.",
                ));
            }
            let pending = state
                .pending
                .values()
                .filter(|pending| pending.prompt.module_id == module_id)
                .count();
            if pending >= MAX_PENDING_PROMPTS {
                return Err(HostError::new(
                    HostErrorCode::Busy,
                    "Too many prompts are waiting for the user.",
                ));
            }
            state.next = state.next.wrapping_add(1);
            let prompt_id = format!("prompt-{}", state.next);
            state.pending.insert(
                prompt_id.clone(),
                Pending {
                    owner,
                    prompt: ModulePrompt {
                        prompt_id: prompt_id.clone(),
                        module_id: module_id.into(),
                        title: request.title,
                        body: request.body,
                        choices: request.choices,
                        expires_at_ms,
                    },
                    answer,
                },
            );
            prompt_id
        };
        self.changed();
        let choice = received.recv_timeout(timeout).ok().flatten();
        if self.lock().pending.remove(&prompt_id).is_some() {
            self.changed();
        }
        Ok(PromptAnswer {
            choice: choice.or_else(|| received.try_recv().ok().flatten()),
        })
    }

    pub(crate) fn list(&self) -> Vec<ModulePrompt> {
        self.lock()
            .pending
            .values()
            .map(|pending| pending.prompt.clone())
            .collect()
    }

    /// `None` dismisses the prompt.
    pub(crate) fn answer(
        &self,
        prompt_id: &str,
        choice: Option<String>,
    ) -> Result<(), ModuleError> {
        let pending = {
            let mut state = self.lock();
            let pending = state.pending.get(prompt_id).ok_or_else(|| {
                ModuleError::Invalid("The prompt is no longer waiting for an answer.".into())
            })?;
            if choice.as_ref().is_some_and(|choice| {
                !pending
                    .prompt
                    .choices
                    .iter()
                    .any(|candidate| &candidate.id == choice)
            }) {
                return Err(ModuleError::Invalid(
                    "The prompt has no such choice.".into(),
                ));
            }
            state.pending.remove(prompt_id)
        };
        if let Some(pending) = pending {
            let _ = pending.answer.try_send(choice);
            self.changed();
        }
        Ok(())
    }

    /// Dismisses every prompt of a module instance that is stopping.
    pub(crate) fn dismiss(&self, owner: u64) {
        let dismissed: Vec<_> = {
            let mut state = self.lock();
            let ids: Vec<_> = state
                .pending
                .iter()
                .filter(|(_, pending)| pending.owner == owner)
                .map(|(id, _)| id.clone())
                .collect();
            ids.iter()
                .filter_map(|id| state.pending.remove(id))
                .collect()
        };
        if dismissed.is_empty() {
            return;
        }
        for pending in dismissed {
            let _ = pending.answer.try_send(None);
        }
        self.changed();
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn changed(&self) {
        let pending = u32::try_from(self.lock().pending.len()).unwrap_or(u32::MAX);
        let listener = self
            .listener
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        if let Some(listener) = listener {
            listener(pending);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use copypaste_module_sdk::PromptChoice;
    use std::sync::atomic::AtomicU32;

    fn request(timeout_secs: u32) -> PromptRequest {
        PromptRequest {
            title: "Allow access?".into(),
            body: "A client asks for history.".into(),
            choices: vec![PromptChoice {
                id: "allow".into(),
                title: "Allow".into(),
                destructive: false,
            }],
            timeout_secs,
        }
    }

    fn wait_for(prompts: &Prompts, count: usize) -> Vec<ModulePrompt> {
        loop {
            let pending = prompts.list();
            if pending.len() == count {
                return pending;
            }
            std::thread::yield_now();
        }
    }

    #[test]
    fn prompts_resolve_by_answer_dismissal_and_stay_bounded_per_module() {
        let prompts = Arc::new(Prompts::default());
        let last = Arc::new(AtomicU32::new(u32::MAX));
        prompts.set_listener({
            let last = Arc::clone(&last);
            Arc::new(move |pending| last.store(pending, Ordering::SeqCst))
        });
        let accepting = Arc::new(AtomicBool::new(true));
        let ask = |module: &'static str| {
            let (prompts, accepting) = (Arc::clone(&prompts), Arc::clone(&accepting));
            std::thread::spawn(move || prompts.ask(module, 1, &accepting, request(300)))
        };
        let answered = ask("a");
        let pending = wait_for(&prompts, 1);
        assert!(prompts
            .answer(&pending[0].prompt_id, Some("other".into()))
            .is_err());
        prompts
            .answer(&pending[0].prompt_id, Some("allow".into()))
            .unwrap();
        assert_eq!(
            answered.join().unwrap().unwrap().choice.as_deref(),
            Some("allow")
        );
        assert_eq!(last.load(Ordering::SeqCst), 0);

        let waiting: Vec<_> = (0..MAX_PENDING_PROMPTS).map(|_| ask("a")).collect();
        wait_for(&prompts, MAX_PENDING_PROMPTS);
        assert_eq!(
            prompts
                .ask("a", 1, &accepting, request(1))
                .unwrap_err()
                .code,
            HostErrorCode::Busy
        );
        accepting.store(false, Ordering::Release);
        prompts.dismiss(1);
        for thread in waiting {
            assert_eq!(thread.join().unwrap().unwrap().choice, None);
        }
        assert!(prompts.list().is_empty());
        assert_eq!(
            prompts
                .ask("a", 1, &accepting, request(1))
                .unwrap_err()
                .code,
            HostErrorCode::Unavailable
        );
        let other = AtomicBool::new(true);
        assert_eq!(
            prompts.ask("b", 2, &other, request(1)).unwrap().choice,
            None
        );
        assert_eq!(last.load(Ordering::SeqCst), 0);
    }
}
