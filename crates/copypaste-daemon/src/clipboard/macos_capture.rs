use std::path::PathBuf;

use tracing::debug;

use super::Capture;

#[derive(Clone)]
pub(super) struct FrontmostApp {
    pub(super) bundle_id: Option<String>,
    pub(super) name: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Attribution {
    Bundle,
    NameOnly,
    Unavailable,
}

impl Attribution {
    pub(super) fn from_app(app: Option<&FrontmostApp>) -> Self {
        match app {
            Some(FrontmostApp {
                bundle_id: Some(_), ..
            }) => Self::Bundle,
            Some(FrontmostApp { name: Some(_), .. }) => Self::NameOnly,
            _ => Self::Unavailable,
        }
    }
}

pub(super) fn first_absolute_filename_plist(bytes: &[u8]) -> Option<PathBuf> {
    if !bytes.starts_with(b"bplist00") {
        debug!("legacy filename pasteboard payload was not a binary plist; dropped");
        return None;
    }
    plist::from_bytes::<Vec<String>>(bytes)
        .ok()?
        .into_iter()
        .map(PathBuf::from)
        .find(|path| path.is_absolute())
}

pub(super) fn file_capture(
    path: PathBuf,
    app_bundle_id: Option<String>,
    app_name: Option<String>,
) -> Option<Capture> {
    if !path.is_absolute() {
        debug!("pasteboard file path was not absolute; dropped");
        return None;
    }
    let filename = path.file_name()?.to_string_lossy().into_owned();
    let mime = mime_guess::from_path(&path)
        .first_raw()
        .unwrap_or("application/octet-stream");
    let metadata = copypaste_core::FileMetadata::new(filename, mime)?;
    Some(Capture {
        content: String::new(),
        binary_content: None,
        file_path: Some(path),
        file_metadata: Some(metadata),
        content_type: copypaste_ipc::content_type::FILE.to_string(),
        app_bundle_id,
        app_name,
    })
}
