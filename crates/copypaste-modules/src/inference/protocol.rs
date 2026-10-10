use crate::ModuleError;
use copypaste_module_sdk::{ModuleInvocation, ModuleOutput, ModuleTarget, MAX_INVOCATION_BYTES};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::PathBuf,
};
use zeroize::Zeroizing;

const MAX_FRAME_BYTES: usize = MAX_INVOCATION_BYTES + 64 * 1024;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WorkerIdentity {
    pub app_version: String,
    pub target: ModuleTarget,
    pub public_key: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InferenceRequest {
    pub identity: WorkerIdentity,
    pub id: String,
    pub package_dir: PathBuf,
    pub data_dir: PathBuf,
    pub invocation: ModuleInvocation,
}

pub(crate) type InferenceReply = Result<ModuleOutput, String>;

pub(crate) fn write_frame(
    writer: &mut impl Write,
    value: &impl Serialize,
) -> Result<(), ModuleError> {
    let bytes = Zeroizing::new(serde_json::to_vec(value).map_err(|_| ModuleError::State)?);
    if bytes.is_empty() || bytes.len() > MAX_FRAME_BYTES {
        return Err(ModuleError::Invalid(
            "The inference message exceeds its limit.".into(),
        ));
    }
    writer.write_all(&(bytes.len() as u32).to_le_bytes())?;
    writer.write_all(&bytes)?;
    writer.flush()?;
    Ok(())
}

pub(crate) fn read_frame<T: DeserializeOwned>(
    reader: &mut impl Read,
) -> Result<Option<T>, ModuleError> {
    let mut length = [0; 4];
    match reader.read(&mut length[..1])? {
        0 => return Ok(None),
        1 => {}
        _ => unreachable!(),
    }
    reader.read_exact(&mut length[1..])?;
    let length = u32::from_le_bytes(length) as usize;
    if length == 0 || length > MAX_FRAME_BYTES {
        return Err(ModuleError::Invalid(
            "The inference message exceeds its limit.".into(),
        ));
    }
    let mut bytes = Zeroizing::new(vec![0; length]);
    reader.read_exact(&mut bytes)?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|_| ModuleError::State)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn framing_rejects_oversized_and_truncated_messages_before_decoding() {
        assert!(
            read_frame::<String>(&mut Cursor::new((MAX_FRAME_BYTES as u32 + 1).to_le_bytes()))
                .is_err()
        );
        assert!(read_frame::<String>(&mut Cursor::new([4, 0, 0, 0, b'"'])).is_err());
        assert!(read_frame::<String>(&mut Cursor::new([1])).is_err());
        assert!(read_frame::<String>(&mut Cursor::new([]))
            .unwrap()
            .is_none());
    }

    #[test]
    fn framing_preserves_unicode_and_multiple_messages() {
        let mut bytes = Vec::new();
        write_frame(&mut bytes, &"Оренда квартири").unwrap();
        write_frame(&mut bytes, &"next").unwrap();
        let mut reader = Cursor::new(bytes);
        assert_eq!(
            read_frame::<String>(&mut reader).unwrap().unwrap(),
            "Оренда квартири"
        );
        assert_eq!(read_frame::<String>(&mut reader).unwrap().unwrap(), "next");
        assert!(read_frame::<String>(&mut reader).unwrap().is_none());
    }
}
