//! Public NSURL decoding stays on the admitted blocking poll worker.

use std::ffi::{CStr, OsString};
use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::ptr::NonNull;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{msg_send, msg_send_id, ClassType};
use objc2_app_kit::{NSPasteboard, NSPasteboardURLReadingFileURLsOnlyKey};
use objc2_foundation::{NSArray, NSDictionary, NSNumber, NSString, NSURL};

use super::super::file_capture::{self, FileReadError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FileInputError {
    DataUnavailable,
    UnsupportedItemCount,
    NativeObjectUnavailable,
    NonLocalUrl,
    ResolutionUnavailable,
    InvalidPath,
    InvalidMetadata,
    GenerationChanged,
    Read(FileReadError),
}

impl FileInputError {
    fn message(self) -> &'static str {
        match self {
            Self::DataUnavailable => "file capture input rejected: native URL data unavailable",
            Self::UnsupportedItemCount => "file capture input rejected: item count unsupported",
            Self::NativeObjectUnavailable => {
                "file capture input rejected: native URL object unavailable"
            }
            Self::NonLocalUrl => "file capture input rejected: URL is not a local file",
            Self::ResolutionUnavailable => {
                "file capture input rejected: native URL resolution unavailable"
            }
            Self::InvalidPath => "file capture input rejected: resolved path invalid",
            Self::InvalidMetadata => "file capture input rejected: file metadata invalid",
            Self::GenerationChanged => {
                "file capture input rejected: generation or coverage changed"
            }
            Self::Read(reason) => reason.message(),
        }
    }
}

pub(super) fn reject(reason: FileInputError) {
    tracing::warn!(message = reason.message());
}

trait FileUrl {
    fn local(&self) -> bool;
    fn start_scope(&self) -> bool;
    fn stop_scope(&self);
    fn path(&self) -> Result<PathBuf, FileInputError>;
}
trait NativeInput {
    type Url: FileUrl;
    fn total_items(&self) -> Option<usize>;
    fn representation_len(&self) -> Option<usize>;
    fn decode(&self) -> Result<Self::Url, FileInputError>;
}

struct Scope<'a, U: FileUrl> {
    url: &'a U,
    active: bool,
}
impl<'a, U: FileUrl> Scope<'a, U> {
    fn new(url: &'a U) -> Self {
        Self {
            url,
            active: url.start_scope(),
        }
    }
}
impl<U: FileUrl> Drop for Scope<'_, U> {
    fn drop(&mut self) {
        if self.active {
            self.url.stop_scope();
        }
    }
}

fn metadata(path: &Path) -> Result<copypaste_core::FileMetadata, FileInputError> {
    if !path.is_absolute() {
        return Err(FileInputError::InvalidPath);
    }
    let reference = path
        .to_str()
        .filter(|path| !path.is_empty())
        .ok_or(FileInputError::InvalidMetadata)?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(FileInputError::InvalidMetadata)?;
    copypaste_core::FileMetadata::with_source_reference(name, "application/octet-stream", reference)
        .ok_or(FileInputError::InvalidMetadata)
}

fn materialize<I: NativeInput>(
    input: &I,
    cap: u64,
    valid: impl Fn() -> bool,
    read: impl FnOnce(&Path, u64) -> Result<Vec<u8>, FileReadError>,
) -> Result<(Vec<u8>, copypaste_core::FileMetadata), FileInputError> {
    let fence = || {
        if valid() {
            Ok(())
        } else {
            Err(FileInputError::GenerationChanged)
        }
    };
    fence()?;
    let count = input.total_items();
    fence()?;
    if count != Some(1) {
        return Err(FileInputError::UnsupportedItemCount);
    }
    let len = input.representation_len();
    fence()?;
    let len = len.ok_or(FileInputError::DataUnavailable)?;
    if len as u64 > cap {
        return Err(FileInputError::Read(FileReadError::TooLarge));
    }
    let url = input.decode();
    fence()?;
    let url = url?;
    if !url.local() {
        return Err(FileInputError::NonLocalUrl);
    }
    let _scope = Scope::new(&url);
    let path = url.path();
    fence()?;
    let path = path?;
    let metadata = metadata(&path)?;
    fence()?;
    let bytes = read(&path, cap);
    fence()?;
    let bytes = bytes.map_err(FileInputError::Read)?;
    Ok((bytes, metadata))
}

struct PasteboardInput<'a> {
    pasteboard: &'a NSPasteboard,
    uti: &'a NSString,
}
impl NativeInput for PasteboardInput<'_> {
    type Url = Retained<NSURL>;
    fn total_items(&self) -> Option<usize> {
        unsafe { self.pasteboard.pasteboardItems().map(|items| items.len()) }
    }
    fn representation_len(&self) -> Option<usize> {
        unsafe {
            self.pasteboard
                .dataForType(self.uti)
                .map(|data| data.length())
        }
    }
    fn decode(&self) -> Result<Self::Url, FileInputError> {
        // SAFETY: The pinned generated binding spells a Class array as TodoClass.
        // These documented selectors receive an NSArray containing only NSURL's
        // Objective-C Class and a file-only NSNumber option. msg_send_id retains
        // the returned arrays. Validate the result's NSURL class before casting;
        // no native object or scope escapes the synchronous poll/autorelease pool.
        unsafe {
            let classes: Retained<NSArray<AnyObject>> =
                msg_send_id![NSArray::<AnyObject>::class(), arrayWithObject: NSURL::class()];
            let file_only = NSNumber::numberWithBool(true);
            let options =
                NSDictionary::from_slice(&[NSPasteboardURLReadingFileURLsOnlyKey], &[&*file_only]);
            let objects: Option<Retained<NSArray<AnyObject>>> =
                msg_send_id![self.pasteboard, readObjectsForClasses: &*classes options: &*options];
            let objects = objects.ok_or(FileInputError::NativeObjectUnavailable)?;
            if objects.len() != 1 {
                return Err(FileInputError::NativeObjectUnavailable);
            }
            let object = objects
                .get_retained(0)
                .ok_or(FileInputError::NativeObjectUnavailable)?;
            let is_url: bool = msg_send![&*object, isKindOfClass: NSURL::class()];
            if !is_url {
                return Err(FileInputError::NativeObjectUnavailable);
            }
            Ok(Retained::cast(object))
        }
    }
}
fn local(url: &NSURL) -> bool {
    unsafe {
        url.isFileURL()
            && url.host().is_none_or(|host| {
                let host = host.to_string();
                host.is_empty() || host.eq_ignore_ascii_case("localhost")
            })
            && url.user().is_none()
            && url.password().is_none()
            && url.port().is_none()
            && url.query().is_none()
            && url.fragment().is_none()
    }
}
impl FileUrl for Retained<NSURL> {
    fn local(&self) -> bool {
        local(self)
    }
    fn start_scope(&self) -> bool {
        unsafe { self.startAccessingSecurityScopedResource() }
    }
    fn stop_scope(&self) {
        unsafe {
            self.stopAccessingSecurityScopedResource();
        }
    }
    fn path(&self) -> Result<PathBuf, FileInputError> {
        unsafe {
            let path_url = self
                .filePathURL()
                .ok_or(FileInputError::ResolutionUnavailable)?;
            if !local(&path_url) {
                return Err(FileInputError::NonLocalUrl);
            }
            if path_url.isFileReferenceURL() {
                return Err(FileInputError::ResolutionUnavailable);
            }
            // Existing FileMetadata reference bound, plus the native NUL terminator.
            let mut buffer = [0xff_u8; 4097];
            if !path_url.getFileSystemRepresentation_maxLength(
                NonNull::new(buffer.as_mut_ptr().cast()).unwrap(),
                buffer.len(),
            ) {
                return Err(FileInputError::InvalidPath);
            }
            let end = buffer
                .iter()
                .position(|byte| *byte == 0)
                .ok_or(FileInputError::InvalidPath)?;
            let bytes = CStr::from_bytes_with_nul(&buffer[..=end])
                .map_err(|_| FileInputError::InvalidPath)?
                .to_bytes();
            if bytes.is_empty() {
                return Err(FileInputError::InvalidPath);
            }
            Ok(PathBuf::from(OsString::from_vec(bytes.to_vec())))
        }
    }
}

pub(super) fn read(
    pasteboard: &NSPasteboard,
    uti: &NSString,
    cap: u64,
    valid: impl Fn() -> bool,
) -> Result<(Vec<u8>, copypaste_core::FileMetadata), FileInputError> {
    materialize(
        &PasteboardInput { pasteboard, uti },
        cap,
        valid,
        file_capture::read,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;
    #[derive(Clone)]
    struct Url {
        local: bool,
        scoped: bool,
        stops: Rc<Cell<usize>>,
        path: Result<PathBuf, FileInputError>,
    }
    impl FileUrl for Url {
        fn local(&self) -> bool {
            self.local
        }
        fn start_scope(&self) -> bool {
            self.scoped
        }
        fn stop_scope(&self) {
            self.stops.set(self.stops.get() + 1);
        }
        fn path(&self) -> Result<PathBuf, FileInputError> {
            self.path.clone()
        }
    }
    struct Input {
        count: Option<usize>,
        len: Option<usize>,
        url: Result<Url, FileInputError>,
        decoded: Cell<usize>,
    }
    impl NativeInput for Input {
        type Url = Url;
        fn total_items(&self) -> Option<usize> {
            self.count
        }
        fn representation_len(&self) -> Option<usize> {
            self.len
        }
        fn decode(&self) -> Result<Url, FileInputError> {
            self.decoded.set(self.decoded.get() + 1);
            self.url.clone()
        }
    }
    fn input(scoped: bool) -> Input {
        Input {
            count: Some(1),
            len: Some(8),
            url: Ok(Url {
                local: true,
                scoped,
                stops: Rc::new(Cell::new(0)),
                path: Ok(PathBuf::from("/local/space λ %#")),
            }),
            decoded: Cell::new(0),
        }
    }
    #[test]
    fn scope_balances_success_resolution_failure_read_failure_and_fence_failure() {
        for scoped in [false, true] {
            for stage in 0..4 {
                let mut input = input(scoped);
                let stops = input.url.as_ref().unwrap().stops.clone();
                if stage == 1 {
                    input.url.as_mut().unwrap().path = Err(FileInputError::ResolutionUnavailable);
                }
                let calls = Cell::new(0);
                let result = materialize(
                    &input,
                    32,
                    || {
                        calls.set(calls.get() + 1);
                        stage != 3 || calls.get() < 7
                    },
                    |_, _| {
                        if stage == 2 {
                            Err(FileReadError::OpenPermissionDenied)
                        } else {
                            Ok(vec![7; 32])
                        }
                    },
                );
                assert_eq!(result.is_ok(), stage == 0);
                assert_eq!(stops.get(), usize::from(scoped));
            }
        }
    }
    #[test]
    fn count_length_and_decode_failures_do_not_open() {
        for stage in 0..4 {
            let mut input = input(true);
            match stage {
                0 => input.count = Some(2),
                1 => input.len = None,
                2 => input.len = Some(33),
                _ => input.url = Err(FileInputError::NativeObjectUnavailable),
            }
            assert!(materialize(
                &input,
                32,
                || true,
                |_, _| panic!("rejected native input must not open")
            )
            .is_err());
            assert_eq!(input.decoded.get(), usize::from(stage == 3));
        }
    }
    #[test]
    fn generation_changes_before_open_and_after_read_drop_owned_bytes() {
        for failing_fence in [4, 5, 6, 7] {
            let input = input(true);
            let fences = Cell::new(0);
            let reads = Cell::new(0);
            assert_eq!(
                materialize(
                    &input,
                    32,
                    || {
                        fences.set(fences.get() + 1);
                        fences.get() != failing_fence
                    },
                    |_, _| {
                        reads.set(reads.get() + 1);
                        Ok(vec![7; 32])
                    }
                ),
                Err(FileInputError::GenerationChanged)
            );
            assert_eq!(reads.get(), usize::from(failing_fence == 7));
        }
    }
    #[test]
    fn strict_metadata_preserves_unicode_and_literal_percent_hash() {
        let value = metadata(Path::new("/local/space λ %#")).unwrap();
        assert_eq!(value.filename, "space λ %#");
        assert_eq!(value.source_reference.as_deref(), Some("/local/space λ %#"));
        assert_eq!(
            metadata(Path::new("relative")),
            Err(FileInputError::InvalidPath)
        );
        assert_eq!(
            metadata(&PathBuf::from(OsString::from_vec(b"/local/\xff".to_vec()))),
            Err(FileInputError::InvalidMetadata)
        );
        assert_eq!(
            metadata(&PathBuf::from(format!("/local/{}", "x".repeat(256)))),
            Err(FileInputError::InvalidMetadata)
        );
    }
    #[test]
    fn nonlocal_and_metadata_failure_stop_before_file_open() {
        let mut input = input(true);
        input.url.as_mut().unwrap().local = false;
        let stops = input.url.as_ref().unwrap().stops.clone();
        assert_eq!(
            materialize(&input, 32, || true, |_, _| panic!("nonlocal must not open")),
            Err(FileInputError::NonLocalUrl)
        );
        assert_eq!(stops.get(), 0);
        input.url.as_mut().unwrap().local = true;
        input.url.as_mut().unwrap().path = Ok("relative".into());
        assert_eq!(
            materialize(
                &input,
                32,
                || true,
                |_, _| panic!("invalid metadata must not open")
            ),
            Err(FileInputError::InvalidPath)
        );
        assert_eq!(stops.get(), 1);
        let reference = format!("/{}", "x/".repeat(2048));
        assert_eq!(
            metadata(Path::new(&reference)),
            Err(FileInputError::InvalidMetadata)
        );
    }
    #[test]
    fn native_and_io_rejection_categories_survive_actual_default_runtime_formatter() {
        use FileInputError::*;
        let reasons = [
            DataUnavailable,
            UnsupportedItemCount,
            NativeObjectUnavailable,
            NonLocalUrl,
            ResolutionUnavailable,
            InvalidPath,
            InvalidMetadata,
            GenerationChanged,
            Read(FileReadError::OpenPermissionDenied),
            Read(FileReadError::ReadPermissionDenied),
        ];
        let (_, log) = copypaste_runtime_log::test_support::capture(|| {
            for reason in reasons {
                reject(reason);
            }
            tracing::warn!(
                native_error = "secret-native-error",
                url = "file:///secret-native-url",
                filename = "secret-native-name",
                bytes = "secret-native-bytes",
                "native formatter boundary control"
            );
        });
        assert_eq!(
            log.matches("file capture input rejected:").count(),
            reasons.len()
        );
        let mut distinct = std::collections::HashSet::new();
        for reason in reasons {
            let message = reason.message();
            assert!(distinct.insert(message));
            assert_eq!(
                log.lines().filter(|line| line.ends_with(message)).count(),
                1
            );
        }
        for token in [
            "secret-native-error",
            "secret-native-url",
            "secret-native-name",
            "secret-native-bytes",
        ] {
            assert!(!log.contains(token));
        }
        assert!(log.contains("native formatter boundary control"));
    }
}
