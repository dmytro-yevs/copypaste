//! Invocation-owned file snapshots outlive client-side file picker leases.
use crate::ModuleError;
use copypaste_module_sdk::{ModuleField, ModuleFieldValue};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::Read,
    path::Path,
};

pub(crate) fn snapshot(
    root: &Path,
    fields: &[ModuleField],
    values: &mut BTreeMap<String, Value>,
) -> Result<Option<tempfile::TempDir>, ModuleError> {
    let mut directory = None;
    let mut total = 0u64;
    for (index, field) in fields.iter().enumerate() {
        let ModuleFieldValue::File {
            accepted_extensions,
            max_bytes,
            ..
        } = &field.value
        else {
            continue;
        };
        let source = values
            .get(&field.id)
            .and_then(Value::as_str)
            .unwrap_or_default();
        if source.is_empty() {
            continue;
        }
        let source = Path::new(source);
        let extension = source
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let metadata = fs::metadata(source)?;
        if !metadata.is_file()
            || metadata.len() > *max_bytes
            || !accepted_extensions.contains(&extension)
        {
            return Err(ModuleError::Invalid(format!(
                "{} is not an accepted file.",
                field.title
            )));
        }
        if directory.is_none() {
            fs::create_dir_all(root.join("inputs"))?;
            directory = Some(tempfile::tempdir_in(root.join("inputs"))?);
        }
        let destination = directory
            .as_ref()
            .ok_or(ModuleError::State)?
            .path()
            .join(format!("input-{index}.{extension}"));
        let mut source = File::open(source)?.take(max_bytes + 1);
        let mut destination_file = File::options()
            .create_new(true)
            .write(true)
            .open(&destination)?;
        let copied = std::io::copy(&mut source, &mut destination_file)?;
        total = total.checked_add(copied).ok_or(ModuleError::State)?;
        if copied > *max_bytes || total > 2 * 1024 * 1024 * 1024 {
            return Err(ModuleError::Invalid(
                "The module file input exceeds its size limit.".into(),
            ));
        }
        values.insert(
            field.id.clone(),
            Value::String(destination.to_string_lossy().into_owned()),
        );
    }
    Ok(directory)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn file_snapshot_survives_source_removal_and_releases_after_invocation() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("original.png");
        fs::write(&source, b"image").unwrap();
        let fields = [ModuleField {
            id: "image".into(),
            title: "Image".into(),
            value: ModuleFieldValue::File {
                accepted_extensions: vec!["png".into()],
                required: true,
                max_bytes: 10,
            },
        }];
        let mut values = BTreeMap::from([(
            "image".into(),
            Value::String(source.to_string_lossy().into_owned()),
        )]);
        let lease = snapshot(root.path(), &fields, &mut values)
            .unwrap()
            .unwrap();
        fs::remove_file(source).unwrap();
        let staged = Path::new(values["image"].as_str().unwrap());
        assert_eq!(fs::read(staged).unwrap(), b"image");
        drop(lease);
        assert!(!staged.exists());
    }
    #[test]
    fn file_type_and_size_are_checked_before_native_code_runs() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("original.jpg");
        fs::write(&source, b"large image").unwrap();
        let fields = [ModuleField {
            id: "image".into(),
            title: "Image".into(),
            value: ModuleFieldValue::File {
                accepted_extensions: vec!["png".into()],
                required: true,
                max_bytes: 1,
            },
        }];
        let mut values = BTreeMap::from([(
            "image".into(),
            Value::String(source.to_string_lossy().into_owned()),
        )]);
        assert!(snapshot(root.path(), &fields, &mut values).is_err());
    }
}
