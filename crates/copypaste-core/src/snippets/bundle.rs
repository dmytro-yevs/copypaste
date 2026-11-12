//! The versioned snippet bundle format and its passphrase encryption.
//!
//! A bundle is one JSON object: `format`, `version`, `collections` and
//! `snippets`, with stable ids and no usage data. An encrypted bundle is a
//! fixed header (magic, Argon2id salt, STREAM nonce) followed by the JSON
//! sealed with XChaCha20-Poly1305 STREAM in 64 KiB chunks, every chunk
//! authenticating the header. Both directions stream, so neither the file
//! nor the whole plaintext is held in memory. Errors carry no content.

use std::io::{self, Read, Write};

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::generic_array::GenericArray;
use chacha20poly1305::aead::stream::{DecryptorBE32, EncryptorBE32};
use chacha20poly1305::aead::Payload;
use chacha20poly1305::{Key, KeyInit, XChaCha20Poly1305};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use crate::storage::snippets::SnippetError;
use crate::storage::StoreError;

pub const FORMAT: &str = "copypaste.snippets";
pub const VERSION: u32 = 1;
/// Largest file an import reads, plaintext or sealed.
pub const MAX_IMPORT_BYTES: u64 = 50 * 1024 * 1024;
pub const MAX_IMPORT_ROWS: usize = 10_000;
/// Counted in characters, so a short passphrase in any script is judged alike.
pub const MIN_PASSPHRASE_CHARS: usize = 8;
/// Collections one bundle may describe; the library itself holds at most 200.
pub const MAX_BUNDLE_COLLECTIONS: usize = 1_000;
/// The look of an imported collection whose file names none.
pub const DEFAULT_COLLECTION_ICON: &str = "folder";
pub const DEFAULT_COLLECTION_ACCENT: &str = "neutral";

/// The last byte is the sealing version; it fixes every parameter below.
const MAGIC: [u8; 8] = *b"CPSNIPS\x01";
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 19;
const HEADER_LEN: usize = MAGIC.len() + SALT_LEN + NONCE_LEN;
const CHUNK: usize = 64 * 1024;
const TAG_LEN: usize = 16;
/// Sealing overhead a sealed file may add over [`MAX_IMPORT_BYTES`].
pub(crate) const SEALED_OVERHEAD: u64 =
    (HEADER_LEN + TAG_LEN * (MAX_IMPORT_BYTES as usize / CHUNK + 1)) as u64;
/// Argon2id at the cloud passphrase's cost (19 MiB, 2 passes, 1 lane): the
/// same floor, paid once per import or export.
const ARGON2_M_COST_KIB: u32 = 19_456;
const ARGON2_T_COST: u32 = 2;
const ARGON2_P_COST: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct BundleCollection {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub accent: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct BundleSnippet {
    pub id: String,
    #[serde(default)]
    pub collection_id: Option<String>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub abbreviation: Option<String>,
    #[serde(default)]
    pub template: bool,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub concealed: bool,
    pub body_text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_rich: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum TransferError {
    #[error("the file could not be read or written")]
    Io(#[from] io::Error),
    #[error("the bundle is encrypted and needs its passphrase")]
    PassphraseRequired,
    #[error("the passphrase does not open this bundle")]
    WrongPassphrase,
    #[error("the passphrase is too short")]
    WeakPassphrase,
    #[error("the encrypted bundle is damaged")]
    Damaged,
    #[error("the file is not in the chosen format")]
    Malformed,
    #[error("the bundle was written by a newer version")]
    UnsupportedVersion,
    #[error("the CSV file has no body column")]
    MissingColumns,
    #[error("the file is larger than an import allows")]
    TooLarge,
    #[error("the file has more snippets than an import allows")]
    TooManyRows,
    #[error("the import was cancelled")]
    Cancelled,
    #[error(transparent)]
    Snippet(#[from] SnippetError),
}

impl From<StoreError> for TransferError {
    fn from(error: StoreError) -> Self {
        Self::Snippet(SnippetError::Store(error))
    }
}

/// Whether a file starting with `prefix` is a sealed bundle.
pub fn is_sealed(prefix: &[u8]) -> bool {
    prefix.starts_with(&MAGIC)
}

pub(crate) fn check_passphrase(passphrase: &str) -> Result<(), TransferError> {
    if passphrase.chars().count() < MIN_PASSPHRASE_CHARS {
        return Err(TransferError::WeakPassphrase);
    }
    Ok(())
}

fn cipher(passphrase: &str, salt: &[u8]) -> Result<XChaCha20Poly1305, TransferError> {
    let params = Params::new(ARGON2_M_COST_KIB, ARGON2_T_COST, ARGON2_P_COST, Some(32))
        .map_err(|_| TransferError::WeakPassphrase)?;
    let mut key = Zeroizing::new([0_u8; 32]);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(passphrase.as_bytes(), salt, key.as_mut())
        .map_err(|_| TransferError::WeakPassphrase)?;
    Ok(XChaCha20Poly1305::new(Key::from_slice(key.as_slice())))
}

fn sealing_failed() -> io::Error {
    io::Error::other("bundle chunk could not be sealed")
}

/// Seals everything written to it. A full chunk is held back until more
/// arrives, so [`SealWriter::finish`] always has a last chunk to mark.
pub(crate) struct SealWriter<W: Write> {
    inner: W,
    stream: Option<EncryptorBE32<XChaCha20Poly1305>>,
    header: [u8; HEADER_LEN],
    pending: Zeroizing<Vec<u8>>,
}

impl<W: Write> SealWriter<W> {
    pub(crate) fn new(mut inner: W, passphrase: &str) -> Result<Self, TransferError> {
        check_passphrase(passphrase)?;
        let mut header = [0_u8; HEADER_LEN];
        header[..MAGIC.len()].copy_from_slice(&MAGIC);
        OsRng.fill_bytes(&mut header[MAGIC.len()..]);
        let cipher = cipher(passphrase, &header[MAGIC.len()..MAGIC.len() + SALT_LEN])?;
        let stream = EncryptorBE32::from_aead(
            cipher,
            GenericArray::from_slice(&header[HEADER_LEN - NONCE_LEN..]),
        );
        inner.write_all(&header)?;
        Ok(Self {
            inner,
            stream: Some(stream),
            header,
            pending: Zeroizing::new(Vec::with_capacity(CHUNK)),
        })
    }

    fn seal_full_chunks(&mut self) -> io::Result<()> {
        while self.pending.len() > CHUNK {
            let stream = self.stream.as_mut().ok_or_else(sealing_failed)?;
            let sealed = stream
                .encrypt_next(Payload {
                    msg: &self.pending[..CHUNK],
                    aad: &self.header,
                })
                .map_err(|_| sealing_failed())?;
            self.inner.write_all(&sealed)?;
            self.pending.drain(..CHUNK);
        }
        Ok(())
    }

    pub(crate) fn finish(mut self) -> io::Result<W> {
        let stream = self.stream.take().ok_or_else(sealing_failed)?;
        let sealed = stream
            .encrypt_last(Payload {
                msg: &self.pending,
                aad: &self.header,
            })
            .map_err(|_| sealing_failed())?;
        self.inner.write_all(&sealed)?;
        self.inner.flush()?;
        Ok(self.inner)
    }
}

impl<W: Write> Write for SealWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.pending.extend_from_slice(bytes);
        self.seal_full_chunks()?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Opens a sealed bundle chunk by chunk. A failure is kept in `failure`, as
/// the parser above it only sees an I/O error.
pub(crate) struct OpenReader<R: Read> {
    inner: R,
    stream: Option<DecryptorBE32<XChaCha20Poly1305>>,
    header: [u8; HEADER_LEN],
    plain: Zeroizing<Vec<u8>>,
    served: usize,
    lookahead: Option<u8>,
    opened_any: bool,
    pub(crate) failure: Option<TransferError>,
}

impl<R: Read> OpenReader<R> {
    pub(crate) fn new(mut inner: R, passphrase: &str) -> Result<Self, TransferError> {
        let mut header = [0_u8; HEADER_LEN];
        inner
            .read_exact(&mut header)
            .map_err(|_| TransferError::Damaged)?;
        if !is_sealed(&header) {
            return Err(TransferError::Malformed);
        }
        let cipher = cipher(passphrase, &header[MAGIC.len()..MAGIC.len() + SALT_LEN])?;
        let stream = DecryptorBE32::from_aead(
            cipher,
            GenericArray::from_slice(&header[HEADER_LEN - NONCE_LEN..]),
        );
        Ok(Self {
            inner,
            stream: Some(stream),
            header,
            plain: Zeroizing::new(Vec::new()),
            served: 0,
            lookahead: None,
            opened_any: false,
            failure: None,
        })
    }

    fn fail(&mut self) -> io::Error {
        self.failure = Some(if self.opened_any {
            TransferError::Damaged
        } else {
            TransferError::WrongPassphrase
        });
        io::Error::new(
            io::ErrorKind::InvalidData,
            "bundle chunk could not be opened",
        )
    }

    /// Reads one sealed chunk and whether the file ends after it.
    fn next_frame(&mut self) -> io::Result<(Vec<u8>, bool)> {
        let mut frame = Vec::with_capacity(CHUNK + TAG_LEN);
        frame.extend(self.lookahead.take());
        let wanted = (CHUNK + TAG_LEN - frame.len()) as u64;
        (&mut self.inner).take(wanted).read_to_end(&mut frame)?;
        let mut next = [0_u8; 1];
        let last = loop {
            match self.inner.read(&mut next) {
                Ok(0) => break true,
                Ok(_) => {
                    self.lookahead = Some(next[0]);
                    break false;
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => return Err(error),
            }
        };
        Ok((frame, last))
    }
}

impl<R: Read> Read for OpenReader<R> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        while self.served == self.plain.len() {
            if self.stream.is_none() {
                return Ok(0);
            }
            let (frame, last) = self.next_frame()?;
            let payload = Payload {
                msg: &frame,
                aad: &self.header,
            };
            let opened = if last {
                self.stream
                    .take()
                    .and_then(|s| s.decrypt_last(payload).ok())
            } else {
                self.stream
                    .as_mut()
                    .and_then(|s| s.decrypt_next(payload).ok())
            };
            let Some(opened) = opened else {
                self.stream = None;
                return Err(self.fail());
            };
            self.opened_any = true;
            self.plain = Zeroizing::new(opened);
            self.served = 0;
        }
        let count = out.len().min(self.plain.len() - self.served);
        out[..count].copy_from_slice(&self.plain[self.served..self.served + count]);
        self.served += count;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(plain: &[u8]) -> Vec<u8> {
        let mut writer = SealWriter::new(Vec::new(), "correct horse").unwrap();
        writer.write_all(plain).unwrap();
        let sealed = writer.finish().unwrap();
        assert!(is_sealed(&sealed));
        let mut opened = Vec::new();
        OpenReader::new(sealed.as_slice(), "correct horse")
            .unwrap()
            .read_to_end(&mut opened)
            .unwrap();
        opened
    }

    #[test]
    fn sealed_chunks_round_trip_at_every_chunk_boundary() {
        for len in [0, 1, CHUNK - 1, CHUNK, CHUNK + 1, 3 * CHUNK] {
            let plain: Vec<u8> = (0..len).map(|at| (at % 251) as u8).collect();
            assert_eq!(round_trip(&plain), plain, "length {len}");
        }
    }

    #[test]
    fn a_wrong_passphrase_or_a_cut_file_is_refused() {
        let mut writer = SealWriter::new(Vec::new(), "correct horse").unwrap();
        writer.write_all(&[7; 2 * CHUNK + 5]).unwrap();
        let sealed = writer.finish().unwrap();

        let mut reader = OpenReader::new(sealed.as_slice(), "wrong horse!").unwrap();
        assert!(reader.read_to_end(&mut Vec::new()).is_err());
        assert!(matches!(
            reader.failure,
            Some(TransferError::WrongPassphrase)
        ));

        let cut = &sealed[..HEADER_LEN + 2 * (CHUNK + TAG_LEN)];
        let mut reader = OpenReader::new(cut, "correct horse").unwrap();
        assert!(reader.read_to_end(&mut Vec::new()).is_err());
        assert!(matches!(reader.failure, Some(TransferError::Damaged)));

        assert!(matches!(
            SealWriter::new(Vec::new(), "short"),
            Err(TransferError::WeakPassphrase)
        ));
    }
}
