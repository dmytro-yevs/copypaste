//! Narrow safe boundary around the macOS Keychain calls that the maintained
//! `security-framework` high-level API cannot express.

#![cfg_attr(not(target_os = "macos"), forbid(unsafe_code))]

#[cfg(target_os = "macos")]
use core_foundation::base::TCFType;
#[cfg(target_os = "macos")]
use core_foundation::boolean::CFBoolean;
#[cfg(target_os = "macos")]
use core_foundation::data::CFData;
#[cfg(target_os = "macos")]
use core_foundation::dictionary::CFDictionary;
#[cfg(target_os = "macos")]
use core_foundation::string::CFString;
#[cfg(target_os = "macos")]
use security_framework::access_control::{ProtectionMode, SecAccessControl};
#[cfg(target_os = "macos")]
use security_framework::base::Error;
#[cfg(target_os = "macos")]
use security_framework_sys::base::{errSecDuplicateItem, errSecSuccess};
#[cfg(target_os = "macos")]
use security_framework_sys::item::{
    kSecAttrAccessControl, kSecAttrAccount, kSecAttrService, kSecAttrSynchronizable, kSecClass,
    kSecClassGenericPassword, kSecValueData,
};
#[cfg(target_os = "macos")]
use security_framework_sys::keychain_item::{SecItemAdd, SecItemUpdate};

/// Add or update a generic password with device-only accessibility and
/// synchronizability explicitly disabled.
#[cfg(target_os = "macos")]
pub fn set_generic_password_locked_down(
    service: &str,
    account: &str,
    secret: &[u8; 32],
) -> Result<(), Error> {
    let access_control = SecAccessControl::create_with_protection(
        Some(ProtectionMode::AccessibleWhenUnlockedThisDeviceOnly),
        0,
    )?;

    let class_key = unsafe { CFString::wrap_under_get_rule(kSecClass) };
    let class_value =
        unsafe { CFString::wrap_under_get_rule(kSecClassGenericPassword).into_CFType() };
    let service_key = unsafe { CFString::wrap_under_get_rule(kSecAttrService) };
    let service_value = CFString::from(service).into_CFType();
    let account_key = unsafe { CFString::wrap_under_get_rule(kSecAttrAccount) };
    let account_value = CFString::from(account).into_CFType();
    let data_key = unsafe { CFString::wrap_under_get_rule(kSecValueData) };
    let data_value = CFData::from_buffer(secret).into_CFType();
    let access_control_key = unsafe { CFString::wrap_under_get_rule(kSecAttrAccessControl) };
    let access_control_value = access_control.into_CFType();
    let synchronizable_key = unsafe { CFString::wrap_under_get_rule(kSecAttrSynchronizable) };
    let synchronizable_value = CFBoolean::false_value().into_CFType();

    let add = CFDictionary::from_CFType_pairs(&[
        (class_key.clone(), class_value.clone()),
        (service_key.clone(), service_value.clone()),
        (account_key.clone(), account_value.clone()),
        (data_key.clone(), data_value.clone()),
        (access_control_key.clone(), access_control_value.clone()),
        (synchronizable_key.clone(), synchronizable_value.clone()),
    ]);
    let status = unsafe { SecItemAdd(add.as_concrete_TypeRef(), std::ptr::null_mut()) };
    if status == errSecSuccess {
        return Ok(());
    }
    if status != errSecDuplicateItem {
        return Err(Error::from_code(status));
    }

    let lookup = CFDictionary::from_CFType_pairs(&[
        (class_key, class_value),
        (service_key, service_value),
        (account_key, account_value),
    ]);
    let update = CFDictionary::from_CFType_pairs(&[
        (data_key, data_value),
        (access_control_key, access_control_value),
        (synchronizable_key, synchronizable_value),
    ]);
    let status =
        unsafe { SecItemUpdate(lookup.as_concrete_TypeRef(), update.as_concrete_TypeRef()) };
    if status == errSecSuccess {
        Ok(())
    } else {
        Err(Error::from_code(status))
    }
}

/// Security attributes returned independently by `SecItemCopyMatching`.
#[cfg(all(target_os = "macos", feature = "test-readback"))]
pub struct GenericPasswordSecurityAttributes {
    pub when_unlocked_this_device_only: bool,
    pub synchronizable: bool,
}

/// Read the attributes used by disposable-Keychain integration tests.
#[cfg(all(target_os = "macos", feature = "test-readback"))]
pub fn generic_password_security_attributes(
    service: &str,
    account: &str,
) -> Result<GenericPasswordSecurityAttributes, Error> {
    use std::ffi::c_void;

    use core_foundation::base::{CFEqual, CFTypeRef};
    use core_foundation::boolean::{kCFBooleanFalse, kCFBooleanTrue};
    use core_foundation::string::CFStringRef;
    use security_framework::item::{ItemClass, ItemSearchOptions, SearchResult};
    use security_framework_sys::access_control::kSecAttrAccessibleWhenUnlockedThisDeviceOnly;
    use security_framework_sys::base::errSecParam;

    extern "C" {
        // security-framework-sys 2.17.0 omits this Security.framework key.
        static kSecAttrAccessible: CFStringRef;
    }

    fn attribute(attributes: &CFDictionary, key: CFStringRef) -> Option<CFTypeRef> {
        attributes.find(key.cast::<c_void>()).map(|value| *value)
    }

    let mut query = ItemSearchOptions::new();
    query
        .class(ItemClass::generic_password())
        .service(service)
        .account(account)
        .load_attributes(true);
    let mut results = query.search()?;
    let Some(SearchResult::Dict(attributes)) = results.pop() else {
        return Err(Error::from_code(errSecParam));
    };

    let accessible = attribute(&attributes, unsafe { kSecAttrAccessible })
        .ok_or_else(|| Error::from_code(errSecParam))?;
    let synchronizable = attribute(&attributes, unsafe { kSecAttrSynchronizable })
        .ok_or_else(|| Error::from_code(errSecParam))?;
    let sync_is_false = unsafe { CFEqual(synchronizable, kCFBooleanFalse.cast()) != 0 };
    let sync_is_true = unsafe { CFEqual(synchronizable, kCFBooleanTrue.cast()) != 0 };
    if !sync_is_false && !sync_is_true {
        return Err(Error::from_code(errSecParam));
    }

    Ok(GenericPasswordSecurityAttributes {
        when_unlocked_this_device_only: unsafe {
            CFEqual(
                accessible,
                kSecAttrAccessibleWhenUnlockedThisDeviceOnly.cast(),
            ) != 0
        },
        synchronizable: sync_is_true,
    })
}
