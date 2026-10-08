//! Bounded downloads authenticated by the installed module's signed manifest.

use crate::ModuleError;
use copypaste_module_sdk::SearchModel;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

pub(crate) fn directory(data: &Path, model: &SearchModel) -> PathBuf {
    data.join("models").join(model.resource_directory_name())
}

pub(crate) fn available(data: &Path, model: &SearchModel) -> bool {
    let root = directory(data, model);
    model.files.iter().all(|file| {
        fs::symlink_metadata(root.join(&file.path)).is_ok_and(|metadata| {
            metadata.is_file()
                && !metadata.file_type().is_symlink()
                && metadata.len() == file.size_bytes
        })
    })
}

pub(crate) fn verify(data: &Path, model: &SearchModel) -> Result<(), ModuleError> {
    let root = directory(data, model);
    for file in &model.files {
        let path = root.join(&file.path);
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.len() != file.size_bytes
        {
            return Err(invalid_model());
        }
        let mut input = fs::File::open(path)?;
        let mut hash = Sha256::new();
        let mut bytes = [0u8; 64 * 1024];
        loop {
            let count = input.read(&mut bytes)?;
            if count == 0 {
                break;
            }
            hash.update(&bytes[..count]);
        }
        if hex::encode(hash.finalize()) != file.sha256.to_ascii_lowercase() {
            return Err(invalid_model());
        }
    }
    Ok(())
}

pub(crate) fn prepare(
    staging: &Path,
    data: &Path,
    model: &SearchModel,
    admitted: impl Fn() -> bool,
) -> Result<Option<PreparedModel>, ModuleError> {
    model.validate()?;
    if available(data, model) && verify(data, model).is_ok() {
        return Ok(None);
    }
    let stage = tempfile::tempdir_in(staging)?;
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(30))
        .timeout(Duration::from_secs(15 * 60))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.url().scheme() != "https" || attempt.previous().len() >= 5 {
                attempt.error("Invalid model redirect")
            } else {
                attempt.follow()
            }
        }))
        .build()
        .map_err(|_| download_failed())?;
    for file in &model.files {
        if !admitted() {
            return Err(ModuleError::Disabled);
        }
        let mut response = client
            .get(&file.url)
            .send()
            .and_then(reqwest::blocking::Response::error_for_status)
            .map_err(|_| download_failed())?;
        if response
            .content_length()
            .is_some_and(|length| length != file.size_bytes)
        {
            return Err(invalid_model());
        }
        let path = stage.path().join(&file.path);
        fs::create_dir_all(path.parent().ok_or(ModuleError::State)?)?;
        let mut output = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)?;
        let mut hash = Sha256::new();
        let mut total = 0u64;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            if !admitted() {
                return Err(ModuleError::Disabled);
            }
            let count = response.read(&mut buffer).map_err(|_| download_failed())?;
            if count == 0 {
                break;
            }
            total = total.checked_add(count as u64).ok_or_else(invalid_model)?;
            if total > file.size_bytes {
                return Err(invalid_model());
            }
            hash.update(&buffer[..count]);
            output.write_all(&buffer[..count])?;
        }
        if total != file.size_bytes
            || hex::encode(hash.finalize()) != file.sha256.to_ascii_lowercase()
        {
            return Err(invalid_model());
        }
        output.sync_all()?;
    }
    Ok(Some(PreparedModel { stage }))
}

pub(crate) struct PreparedModel {
    stage: tempfile::TempDir,
}
impl PreparedModel {
    pub(crate) fn publish(self, data: &Path, model: &SearchModel) -> Result<(), ModuleError> {
        let destination = directory(data, model);
        fs::create_dir_all(destination.parent().ok_or(ModuleError::State)?)?;
        if destination.exists() {
            fs::remove_dir_all(&destination)?;
        }
        fs::rename(self.stage.path(), destination)?;
        Ok(())
    }
}

fn invalid_model() -> ModuleError {
    ModuleError::Invalid("The downloaded language model failed verification.".into())
}
fn download_failed() -> ModuleError {
    ModuleError::Invalid(
        "The language model could not be downloaded. Your previous settings are unchanged.".into(),
    )
}
