//! Real `UNUserNotificationCenter` — the desktop notification plugin always
//! answers Granted and never raises TCC.

#![allow(unsafe_code)]

use std::ptr::NonNull;
use std::sync::mpsc;
use std::time::Duration;

use block2::RcBlock;
use objc2::runtime::Bool;
use objc2_foundation::NSError;
use objc2_user_notifications::{
    UNAuthorizationOptions, UNAuthorizationStatus, UNNotificationSettings, UNUserNotificationCenter,
};

use super::policy::Authorization;
use crate::backend::BackendError;

const MSG_REQUEST: &str = "CopyPaste couldn't ask for notifications.";
const TIMEOUT: Duration = Duration::from_secs(60);

pub fn authorization() -> Result<Authorization, BackendError> {
    let (tx, rx) = mpsc::channel();
    unsafe {
        let center = UNUserNotificationCenter::currentNotificationCenter();
        let tx = tx.clone();
        let block = RcBlock::new(move |settings: NonNull<UNNotificationSettings>| {
            let status = settings.as_ref().authorizationStatus();
            let _ = tx.send(status);
        });
        center.getNotificationSettingsWithCompletionHandler(&block);
    }
    let status = rx
        .recv_timeout(TIMEOUT)
        .map_err(|_| BackendError::Internal(MSG_REQUEST.into()))?;
    Ok(map_status(status))
}

pub fn request() -> Result<(), BackendError> {
    let (tx, rx) = mpsc::channel();
    unsafe {
        let center = UNUserNotificationCenter::currentNotificationCenter();
        let tx = tx.clone();
        let block = RcBlock::new(move |granted: Bool, _error: *mut NSError| {
            let _ = tx.send(granted.as_bool());
        });
        center.requestAuthorizationWithOptions_completionHandler(
            UNAuthorizationOptions::UNAuthorizationOptionAlert
                .union(UNAuthorizationOptions::UNAuthorizationOptionSound)
                .union(UNAuthorizationOptions::UNAuthorizationOptionBadge),
            &block,
        );
    }
    let _ = rx
        .recv_timeout(TIMEOUT)
        .map_err(|_| BackendError::Internal(MSG_REQUEST.into()))?;
    Ok(())
}

fn map_status(status: UNAuthorizationStatus) -> Authorization {
    match status {
        UNAuthorizationStatus::Denied => Authorization::Denied,
        UNAuthorizationStatus::NotDetermined => Authorization::NotDetermined,
        _ => Authorization::Granted,
    }
}
