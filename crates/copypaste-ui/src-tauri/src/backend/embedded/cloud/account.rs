use std::sync::{Arc, Mutex, MutexGuard};

use tokio_util::sync::CancellationToken;

use super::Driver;

pub(super) struct Account {
    pub(super) email: String,
    pub(super) user_id: String,
    pub(super) driver: Arc<Driver>,
    pub(super) cancel: CancellationToken,
}

#[derive(Default)]
pub(super) struct AccountSlot(Mutex<Option<Account>>);

impl AccountSlot {
    pub(super) fn lock(&self) -> MutexGuard<'_, Option<Account>> {
        self.0.lock().unwrap_or_else(|error| error.into_inner())
    }

    pub(super) fn driver(&self) -> Option<Arc<Driver>> {
        self.lock().as_ref().map(|value| Arc::clone(&value.driver))
    }

    pub(super) fn round(&self) -> Option<(Arc<Driver>, CancellationToken)> {
        self.lock()
            .as_ref()
            .map(|value| (Arc::clone(&value.driver), value.cancel.clone()))
    }

    pub(super) fn install(&self, account: Account) {
        let previous = self.lock().replace(account);
        if let Some(previous) = previous {
            previous.cancel.cancel();
        }
    }

    pub(super) fn cancel(&self) {
        if let Some(account) = self.lock().as_ref() {
            account.cancel.cancel();
        }
    }

    pub(super) fn interrupt(&self) {
        if let Some(account) = self.lock().as_mut() {
            account.cancel.cancel();
            account.cancel = CancellationToken::new();
        }
    }

    pub(super) fn take(&self) -> Option<Account> {
        let account = self.lock().take();
        if let Some(account) = account.as_ref() {
            account.cancel.cancel();
        }
        account
    }

    pub(super) fn with_driver<T>(
        &self,
        expected: &Arc<Driver>,
        cancel: &CancellationToken,
        f: impl FnOnce() -> T,
    ) -> Option<T> {
        let account = self.lock();
        account
            .as_ref()
            .filter(|value| Arc::ptr_eq(&value.driver, expected) && !cancel.is_cancelled())?;
        Some(f())
    }
}
