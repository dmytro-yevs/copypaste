//! Binary items envelope: a JSON metadata header, then raw payload bytes, so
//! no format pays JSON escaping or a number-array encoding.
//!
//! Layout (big-endian): magic `CPI1` (4, not valid JSON), total envelope
//! length (4), header length (4), JSON header
//! `{"items":[metadata…],"payloads":[[len,binary]…]}`, then the payloads
//! concatenated in item order.
//!
//! The total length lets a receiver allocate once. Noise authenticates every
//! byte; bounds are checked before payloads are copied.

use std::ops::Range;

use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, Zeroizing};

use super::codec::json_len;
use super::{ProtocolError, SyncItem, MAX_CONTENT_BYTES, MAX_MESSAGE_BYTES};

pub(super) const MAGIC: [u8; 4] = *b"CPI1";
const PREFIX_BYTES: usize = 12;

/// One payload's length and whether it is binary rather than UTF-8 text.
#[derive(Serialize, Deserialize)]
struct Payload(u32, bool);

#[derive(Serialize)]
struct HeaderOut<'a> {
    items: &'a [SyncItem],
    payloads: Vec<Payload>,
}

#[derive(Deserialize)]
struct HeaderIn {
    items: Vec<SyncItem>,
    payloads: Vec<Payload>,
}

fn payload(item: &SyncItem) -> (&[u8], bool) {
    if item.binary_content.is_empty() {
        (item.content.as_bytes(), false)
    } else {
        (&item.binary_content, true)
    }
}

fn invalid() -> ProtocolError {
    ProtocolError::Decode("invalid items envelope".into())
}

/// The envelope as its header plus borrowed payloads, so a sender writes the
/// payloads without first copying them into one buffer.
pub(crate) struct ItemsFrame<'a> {
    head: Zeroizing<Vec<u8>>,
    payloads: Vec<&'a [u8]>,
    total: usize,
}

impl<'a> ItemsFrame<'a> {
    pub(super) fn new(items: &'a [SyncItem]) -> Result<Self, ProtocolError> {
        let mut table = Vec::with_capacity(items.len());
        let mut payloads = Vec::with_capacity(items.len());
        let mut body = 0usize;
        for item in items {
            let (bytes, binary) = payload(item);
            let len = u32::try_from(bytes.len()).map_err(|_| ProtocolError::ContentTooLarge {
                bytes: bytes.len(),
                max: MAX_CONTENT_BYTES,
            })?;
            table.push(Payload(len, binary));
            body = body.saturating_add(bytes.len());
            payloads.push(bytes);
        }
        let header = HeaderOut {
            items,
            payloads: table,
        };
        let header_len = json_len(&header)?;
        let total = PREFIX_BYTES.saturating_add(header_len).saturating_add(body);
        if total > MAX_MESSAGE_BYTES {
            return Err(ProtocolError::MessageTooLarge {
                bytes: total,
                max: MAX_MESSAGE_BYTES,
            });
        }
        let mut head = Zeroizing::new(Vec::with_capacity(PREFIX_BYTES + header_len));
        head.extend_from_slice(&MAGIC);
        head.extend_from_slice(&(total as u32).to_be_bytes());
        head.extend_from_slice(&(header_len as u32).to_be_bytes());
        serde_json::to_writer(&mut *head, &header)
            .map_err(|e| ProtocolError::Encode(e.to_string()))?;
        Ok(Self {
            head,
            payloads,
            total,
        })
    }

    /// Every byte of the envelope, in order.
    pub(crate) fn parts(&self) -> Vec<&[u8]> {
        std::iter::once(self.head.as_slice())
            .chain(self.payloads.iter().copied())
            .collect()
    }

    pub(super) fn to_vec(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.total);
        for part in self.parts() {
            bytes.extend_from_slice(part);
        }
        bytes
    }
}

/// The total length an envelope announces in its first bytes, if they start
/// one. Advisory: [`decode`] still requires it to match exactly.
pub(super) fn announced_len(first: &[u8]) -> Option<usize> {
    let length = first.strip_prefix(&MAGIC)?.get(..4)?;
    Some(u32::from_be_bytes(length.try_into().ok()?) as usize)
}

/// Items without payloads, and each payload's byte range and kind.
type Layout = (Vec<SyncItem>, Vec<(Range<usize>, bool)>);

fn layout(bytes: &[u8]) -> Result<Layout, ProtocolError> {
    let field = |at: usize| -> Result<usize, ProtocolError> {
        let raw = bytes.get(at..at + 4).ok_or_else(invalid)?;
        Ok(u32::from_be_bytes(raw.try_into().map_err(|_| invalid())?) as usize)
    };
    if field(4)? != bytes.len() {
        return Err(invalid());
    }
    let header_end = PREFIX_BYTES
        .checked_add(field(8)?)
        .filter(|&end| end <= bytes.len())
        .ok_or_else(invalid)?;
    // A serde message can quote header values such as file names.
    let HeaderIn { items, payloads } =
        serde_json::from_slice(&bytes[PREFIX_BYTES..header_end]).map_err(|_| invalid())?;
    if payloads.len() != items.len() {
        return Err(invalid());
    }
    let mut ranges = Vec::with_capacity(payloads.len());
    let mut at = header_end;
    for Payload(len, binary) in payloads {
        let len = len as usize;
        if len > MAX_CONTENT_BYTES {
            return Err(ProtocolError::ContentTooLarge {
                bytes: len,
                max: MAX_CONTENT_BYTES,
            });
        }
        let end = at
            .checked_add(len)
            .filter(|&end| end <= bytes.len())
            .ok_or_else(invalid)?;
        ranges.push((at..end, binary));
        at = end;
    }
    if at != bytes.len() {
        return Err(invalid());
    }
    Ok((items, ranges))
}

fn fill(item: &mut SyncItem, bytes: Vec<u8>, binary: bool) -> Result<(), ProtocolError> {
    if binary {
        item.binary_content = bytes;
        return Ok(());
    }
    item.content = String::from_utf8(bytes).map_err(|error| {
        error.into_bytes().zeroize();
        ProtocolError::Decode("item text is not UTF-8".into())
    })?;
    Ok(())
}

pub(super) fn decode(bytes: &[u8]) -> Result<Vec<SyncItem>, ProtocolError> {
    let (mut items, ranges) = layout(bytes)?;
    for (item, (range, binary)) in items.iter_mut().zip(ranges) {
        fill(item, bytes[range].to_vec(), binary)?;
    }
    Ok(items)
}

/// As [`decode`], but a payload filling at least half of `raw`'s allocation
/// takes that allocation over instead of being copied. Smaller payloads are
/// copied so a small item never pins a large buffer.
pub(super) fn decode_owned(raw: &mut Zeroizing<Vec<u8>>) -> Result<Vec<SyncItem>, ProtocolError> {
    let (mut items, ranges) = layout(raw)?;
    let reused = ranges
        .iter()
        .enumerate()
        .max_by_key(|(_, (range, _))| range.len())
        .filter(|(_, (range, _))| range.len() >= raw.capacity() / 2)
        .map(|(index, _)| index);
    for (index, (range, binary)) in ranges.iter().enumerate() {
        if Some(index) != reused {
            fill(&mut items[index], raw[range.clone()].to_vec(), *binary)?;
        }
    }
    if let Some(index) = reused {
        let (range, binary) = ranges[index].clone();
        let mut bytes = std::mem::take(&mut **raw);
        bytes.truncate(range.end);
        bytes.drain(..range.start);
        // The shift leaves stale header and payload bytes past the new length.
        bytes.spare_capacity_mut().zeroize();
        fill(&mut items[index], bytes, binary)?;
    }
    Ok(items)
}

#[cfg(test)]
mod tests {
    use zeroize::Zeroizing;

    use crate::protocol::{plaintext_content_hash, ProtocolError, SyncItem, SyncMessage};

    fn item(id: &str, content_type: &str) -> SyncItem {
        SyncItem {
            item_id: id.into(),
            content: String::new(),
            binary_content: Vec::new(),
            payload_metadata: None,
            source_app_bundle_id: Some("com.example.app".into()),
            source_app_name: None,
            content_type: content_type.into(),
            created_at: 1_000,
            deleted: false,
            content_hash: plaintext_content_hash(b""),
            origin_device_id: "dev-a".into(),
            pinned: false,
            pin_order: None,
            pin_updated_at: 0,
        }
    }

    fn envelope(header: &str, payload: &[u8]) -> Vec<u8> {
        let total = (super::PREFIX_BYTES + header.len() + payload.len()) as u32;
        [
            &super::MAGIC[..],
            &total.to_be_bytes(),
            &(header.len() as u32).to_be_bytes(),
            header.as_bytes(),
            payload,
        ]
        .concat()
    }

    #[test]
    fn every_payload_format_travels_as_raw_bytes() {
        let mut text = item("text", "text/html");
        text.content = "<p>\"quoted\"\n\0</p>".into();
        let mut image = item("image", "image/png");
        image.binary_content = (0..=255).cycle().take(70_000).collect();
        let tombstone = SyncItem {
            deleted: true,
            ..item("gone", "text")
        };
        let message = SyncMessage::Items {
            items: vec![text.clone(), image.clone(), tombstone],
        };

        let encoded = message.encode().unwrap();
        assert_eq!(encoded.capacity(), encoded.len());
        let frame = message.items_frame().unwrap().unwrap();
        assert_eq!(frame.parts().concat(), encoded);
        assert_eq!(SyncMessage::announced_len(&encoded), Some(encoded.len()));
        let payload = text.content.len() + image.binary_content.len();
        let header = u32::from_be_bytes(encoded[8..12].try_into().unwrap()) as usize;
        assert_eq!(encoded.len(), super::PREFIX_BYTES + header + payload);

        assert_eq!(SyncMessage::decode(&encoded).unwrap(), message);
        let owned = SyncMessage::decode_owned(Zeroizing::new(encoded)).unwrap();
        assert_eq!(owned, message);
    }

    #[test]
    fn inconsistent_envelopes_are_refused() {
        let header = r#"{"items":[{"item_id":"a","content_type":"text","created_at":-9,"deleted":false,"content_hash":"h","origin_device_id":"d","pinned":false,"pin_order":null}],"payloads":[[1,false]]}"#;
        let SyncMessage::Items { items } = SyncMessage::decode(&envelope(header, b"x")).unwrap()
        else {
            panic!("wrong variant");
        };
        assert_eq!(items[0].created_at, 0, "negative stamp is clamped");

        let mut wrong_total = envelope(header, b"x");
        wrong_total[7] ^= 1;
        for bytes in [
            envelope(header, b""),
            envelope(header, b"xy"),
            envelope(header, b"\xff"),
            wrong_total,
            br#"{"t":"items","items":[]}"#.to_vec(),
        ] {
            assert!(matches!(
                SyncMessage::decode(&bytes),
                Err(ProtocolError::Decode(_))
            ));
        }
    }
}
