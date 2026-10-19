//! Portable validation of authenticated clipboard payload metadata.
#![forbid(unsafe_code)]
use base64::{engine::general_purpose::STANDARD, Engine as _};
use image::{GenericImageView, ImageFormat, ImageReader, Limits};
use serde::{Deserialize, Serialize};
use std::io::Cursor;

/// User-facing attributes of an opaque file payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileMetadata {
    pub filename: String,
    pub mime_type: String,
    /// The original platform locator captured with the file. Desktop capture
    /// stores an absolute path; Android stores the provider's `content://` URI.
    /// It is optional so file clips written by older versions remain readable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_reference: Option<String>,
    /// An authenticated offer; ordinary history sync carries no file bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transfer: Option<FileTransfer>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileTransfer {
    pub size_bytes: u64,
    pub sha256: String,
}

impl FileTransfer {
    pub fn is_valid(&self) -> bool {
        self.size_bytes <= i64::MAX as u64
            && self.sha256.len() == 64
            && self
                .sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    }
}

impl FileMetadata {
    pub fn new(filename: impl Into<String>, mime_type: impl Into<String>) -> Option<Self> {
        Self::build(filename.into(), mime_type.into(), None)
    }

    pub fn with_source_reference(
        filename: impl Into<String>,
        mime_type: impl Into<String>,
        source_reference: impl Into<String>,
    ) -> Option<Self> {
        Self::build(
            filename.into(),
            mime_type.into(),
            Some(source_reference.into()),
        )
    }

    fn build(
        filename: String,
        mime_type: String,
        source_reference: Option<String>,
    ) -> Option<Self> {
        (filename.len() <= 255
            && !filename.is_empty()
            && std::path::Path::new(&filename)
                .file_name()
                .is_some_and(|name| name == std::ffi::OsStr::new(&filename))
            && mime_type.len() <= 255
            && mime_type.contains('/')
            && source_reference.as_ref().is_none_or(|reference| {
                !reference.is_empty() && reference.len() <= 4096 && !reference.contains('\0')
            }))
        .then_some(Self {
            filename,
            mime_type,
            source_reference,
            transfer: None,
        })
    }

    /// Parse metadata received from an authenticated transport. `Deserialize`
    /// alone is not sufficient: it bypasses the constructor's basename rule.
    #[must_use]
    pub fn from_json(value: &str) -> Option<Self> {
        serde_json::from_str::<Self>(value)
            .ok()
            .filter(|metadata| metadata.is_valid())
    }

    #[must_use]
    pub fn is_valid(&self) -> bool {
        Self::build(
            self.filename.clone(),
            self.mime_type.clone(),
            self.source_reference.clone(),
        )
        .is_some()
            && self.transfer.as_ref().is_none_or(FileTransfer::is_valid)
    }
}

/// Strict, optional application-identity icon carried in signed metadata.
/// The largest padded standard-base64 spelling of a bounded PNG. This is
/// checked before decoding so an authenticated peer still cannot make us
/// allocate an arbitrary icon buffer.
const MAX_SOURCE_APP_ICON_BASE64_BYTES: usize =
    copypaste_ipc::MAX_SOURCE_APP_ICON_BYTES.div_ceil(3) * 4;
/// The 128px RGBA output needs 64 KiB. Leave bounded headroom for decoder
/// state so a valid maximum-sized PNG does not fail solely on scratch space.
const MAX_SOURCE_APP_ICON_DECODED_BYTES: u64 = 256 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceAppIconMetadata {
    pub png_base64: String,
    pub width: u32,
    pub height: u32,
}

impl SourceAppIconMetadata {
    pub fn new(png: &[u8], width: u32, height: u32) -> Option<Self> {
        valid_icon_dimensions(png, width, height).then(|| Self {
            png_base64: STANDARD.encode(png),
            width,
            height,
        })
    }

    pub fn png(&self) -> Option<Vec<u8>> {
        if self.png_base64.len() > MAX_SOURCE_APP_ICON_BASE64_BYTES {
            return None;
        }
        let png = STANDARD.decode(&self.png_base64).ok()?;
        valid_icon_dimensions(&png, self.width, self.height).then_some(png)
    }
}

fn valid_icon_dimensions(png: &[u8], width: u32, height: u32) -> bool {
    if png.is_empty()
        || png.len() > copypaste_ipc::MAX_SOURCE_APP_ICON_BYTES
        || !(1..=copypaste_ipc::SOURCE_APP_ICON_EDGE).contains(&width)
        || !(1..=copypaste_ipc::SOURCE_APP_ICON_EDGE).contains(&height)
    {
        return false;
    }

    let mut reader = ImageReader::with_format(Cursor::new(png), ImageFormat::Png);
    let mut limits = Limits::default();
    limits.max_image_width = Some(copypaste_ipc::SOURCE_APP_ICON_EDGE);
    limits.max_image_height = Some(copypaste_ipc::SOURCE_APP_ICON_EDGE);
    limits.max_alloc = Some(MAX_SOURCE_APP_ICON_DECODED_BYTES);
    reader.limits(limits);
    reader
        .decode()
        .ok()
        .is_some_and(|image| image.dimensions() == (width, height))
}

/// An explicit user choice outranks producer hints; ties prefer concealment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SecretOverride {
    pub updated_at: i64,
    pub secret: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct PayloadMetadata {
    #[serde(
        default,
        skip_serializing_if = "copypaste_ipc::ClipboardPrivacy::is_empty"
    )]
    pub privacy: copypaste_ipc::ClipboardPrivacy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret_override: Option<SecretOverride>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<FileMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_app_icon: Option<SourceAppIconMetadata>,
}

impl PayloadMetadata {
    pub fn set_secret(&mut self, secret: bool, updated_at: i64) {
        self.secret_override = Some(SecretOverride { updated_at, secret });
        self.privacy.secret = secret;
    }

    /// Preserve automatic restrictions unless a newer explicit choice exists.
    pub fn merge_privacy(&mut self, previous: &Self) {
        self.privacy = self.privacy.union(previous.privacy);
        self.secret_override = self.secret_override.max(previous.secret_override);
        if let Some(choice) = self.secret_override {
            self.privacy.secret = choice.secret;
        }
    }

    /// Read only explicit producer hints. Invalid artwork, legacy fields, and
    /// unreadable metadata are not evidence that the clipboard is confidential.
    /// Optional metadata validation must also never erase a valid privacy hint.
    #[must_use]
    pub fn privacy_from_json(value: &str) -> copypaste_ipc::ClipboardPrivacy {
        if value.len() > copypaste_ipc::MAX_SYNC_METADATA_BYTES {
            return Default::default();
        }
        let value = serde_json::from_str::<serde_json::Value>(value).unwrap_or_default();
        let privacy = &value["privacy"];
        copypaste_ipc::ClipboardPrivacy {
            secret: privacy["secret"].as_bool() == Some(true),
            transient: privacy["transient"].as_bool() == Some(true),
        }
    }

    #[must_use]
    pub fn new(
        file: Option<FileMetadata>,
        source_app_icon: Option<SourceAppIconMetadata>,
    ) -> Option<Self> {
        let metadata = Self {
            privacy: Default::default(),
            secret_override: None,
            file,
            source_app_icon,
        };
        metadata.has_valid_components().then_some(metadata)
    }

    #[must_use]
    pub fn is_valid_for(&self, content_type: &str) -> bool {
        self.has_valid_components()
            && if content_type == copypaste_ipc::content_type::FILE {
                self.file.is_some()
            } else {
                (copypaste_ipc::content_type::classify(content_type)
                    == copypaste_ipc::ContentClass::Image
                    && self.file.is_some())
                    || (self.file.is_none()
                        && (self.source_app_icon.is_some()
                            || !self.privacy.is_empty()
                            || self.secret_override.is_some()))
            }
    }

    fn has_valid_components(&self) -> bool {
        let file_is_valid = self.file.as_ref().is_none_or(FileMetadata::is_valid);
        let icon_is_valid = self
            .source_app_icon
            .as_ref()
            .is_none_or(|icon| icon.png().is_some());
        file_is_valid
            && icon_is_valid
            && self
                .secret_override
                .is_none_or(|choice| choice.updated_at > 0 && choice.secret == self.privacy.secret)
            && (self.file.is_some()
                || self.source_app_icon.is_some()
                || !self.privacy.is_empty()
                || self.secret_override.is_some())
    }

    #[must_use]
    pub fn to_json(&self, content_type: &str) -> Option<String> {
        self.is_valid_for(content_type)
            .then(|| serde_json::to_string(self).ok())
            .flatten()
            .filter(|json| json.len() <= copypaste_ipc::MAX_SYNC_METADATA_BYTES)
    }

    pub fn from_json(value: &str, content_type: &str) -> Option<Self> {
        if value.len() > copypaste_ipc::MAX_SYNC_METADATA_BYTES {
            return None;
        }
        if let Some(file) = FileMetadata::from_json(value) {
            return (content_type == copypaste_ipc::content_type::FILE).then_some(Self {
                privacy: Default::default(),
                secret_override: None,
                file: Some(file),
                source_app_icon: None,
            });
        }
        let metadata: Self = serde_json::from_str(value).ok()?;
        metadata.is_valid_for(content_type).then_some(metadata)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};

    #[test]
    fn privacy_uses_explicit_boolean_hints_independently_of_optional_metadata() {
        for value in [
            "not JSON",
            "null",
            "[]",
            r#"{"legacy_private":true}"#,
            r#"{"privacy":{"secret":"true","transient":1}}"#,
            r#"{"source_app_icon":{"png_base64":"broken","width":1,"height":1}}"#,
        ] {
            assert!(
                PayloadMetadata::privacy_from_json(value).is_empty(),
                "{value}"
            );
        }
        let privacy = PayloadMetadata::privacy_from_json(
            r#"{"privacy":{"secret":true,"transient":"bad"},"file":{},"future_field":true}"#,
        );
        assert!(privacy.secret);
        assert!(!privacy.transient);
        let privacy = PayloadMetadata::privacy_from_json(
            r#"{"privacy":{"secret":false,"transient":true},"source_app_icon":"broken"}"#,
        );
        assert!(!privacy.secret);
        assert!(privacy.transient);
    }

    #[test]
    fn manual_privacy_choices_round_trip_and_resolve_by_stamp_then_secrecy() {
        let mut visible = PayloadMetadata::default();
        visible.set_secret(false, 20);
        let json = visible.to_json("text").unwrap();
        assert_eq!(
            PayloadMetadata::from_json(&json, "text"),
            Some(visible.clone())
        );
        let mut secret = PayloadMetadata::default();
        secret.set_secret(true, 10);
        secret.privacy.transient = true;
        visible.merge_privacy(&secret);
        assert!(!visible.privacy.secret);
        assert!(visible.privacy.transient);
        secret.set_secret(true, 20);
        visible.merge_privacy(&secret);
        assert!(visible.privacy.secret);
        secret.set_secret(false, 21);
        visible.merge_privacy(&secret);
        assert!(!visible.privacy.secret);
        assert!(visible.privacy.transient);
        visible.privacy.secret = true;
        assert!(visible.to_json("text").is_none());
    }

    fn png(width: u32, height: u32) -> Vec<u8> {
        let image = DynamicImage::ImageRgba8(ImageBuffer::<Rgba<u8>, Vec<u8>>::new(width, height));
        let mut bytes = Cursor::new(Vec::new());
        image.write_to(&mut bytes, ImageFormat::Png).unwrap();
        bytes.into_inner()
    }
    #[test]
    fn transport_metadata_rejects_a_path_in_the_filename() {
        assert!(FileMetadata::from_json(
            r#"{"filename":"../private.txt","mime_type":"text/plain"}"#
        )
        .is_none());
    }

    #[test]
    fn file_metadata_preserves_a_bounded_source_reference() {
        let metadata = FileMetadata::with_source_reference(
            "private.txt",
            "text/plain",
            "/Users/person/Documents/private.txt",
        )
        .unwrap();
        let json = serde_json::to_string(&metadata).unwrap();

        assert_eq!(FileMetadata::from_json(&json), Some(metadata));
        assert!(
            FileMetadata::with_source_reference("private.txt", "text/plain", "x".repeat(4097),)
                .is_none()
        );
    }

    #[test]
    fn source_icon_envelope_is_strict_and_preserves_legacy_file_metadata() {
        let icon = SourceAppIconMetadata::new(&png(64, 128), 64, 128).expect("valid icon");
        let file = FileMetadata::new("report.pdf", "application/pdf").unwrap();
        let metadata = PayloadMetadata {
            secret_override: None,
            privacy: Default::default(),
            file: Some(file.clone()),
            source_app_icon: Some(icon.clone()),
        };
        let json = metadata.to_json(copypaste_ipc::content_type::FILE).unwrap();
        assert_eq!(
            PayloadMetadata::from_json(&json, copypaste_ipc::content_type::FILE),
            Some(metadata)
        );

        let icon_only = PayloadMetadata {
            secret_override: None,
            privacy: Default::default(),
            file: None,
            source_app_icon: Some(icon),
        };
        assert!(icon_only.to_json("text").is_some());
        let constructed = PayloadMetadata::new(None, icon_only.source_app_icon.clone()).unwrap();
        let constructed_json = constructed.to_json("text").unwrap();
        assert_eq!(
            PayloadMetadata::from_json(&constructed_json, "text"),
            Some(constructed)
        );
        assert_eq!(
            PayloadMetadata::from_json(
                r#"{"filename":"report.pdf","mime_type":"application/pdf"}"#,
                copypaste_ipc::content_type::FILE,
            )
            .and_then(|metadata| metadata.file),
            Some(file)
        );
        assert!(PayloadMetadata::from_json(
            r#"{"source_app_icon":null,"unexpected":true}"#,
            "text",
        )
        .is_none());
    }

    #[test]
    fn native_icon_dimensions_from_windows_and_android_are_preserved() {
        for edge in [16, 32, 48, 64, 96, 128] {
            let icon = SourceAppIconMetadata::new(&png(edge, edge), edge, edge)
                .expect("bounded native icons need no artificial upscaling");
            assert_eq!((icon.width, icon.height), (edge, edge));
        }
    }

    #[test]
    fn source_icon_uses_decoder_dimensions_and_bounded_base64() {
        let actual_oversize = png(129, 64);
        assert!(SourceAppIconMetadata::new(&actual_oversize, 64, 64).is_none());
        assert!(SourceAppIconMetadata::new(&png(128, 128), 128, 128).is_some());

        let oversized = format!(
            r#"{{"source_app_icon":{{"png_base64":"{}","width":64,"height":64}}}}"#,
            "A".repeat(MAX_SOURCE_APP_ICON_BASE64_BYTES + 1),
        );
        assert!(PayloadMetadata::from_json(&oversized, "text").is_none());
        assert!(PayloadMetadata::from_json(
            r#"{"source_app_icon":{"png_base64":"AAAA","width":64,"height":64}}"#,
            copypaste_ipc::content_type::FILE,
        )
        .is_none());
    }
}
