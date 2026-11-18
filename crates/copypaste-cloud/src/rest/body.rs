//! Response bodies read under a byte cap, never held twice.
//!
//! A body goes from the socket into one buffer. A page of rows is split into
//! its elements as it arrives, so each row's bytes are released once that row
//! is parsed, and a page stops being read once it holds [`PAGE_BYTES`].

use base64::encoded_len;
use copypaste_ipc::{MAX_CONTENT_BYTES, MAX_SYNC_METADATA_BYTES};
use reqwest::Response;

use super::error::RestError;
use super::item::CloudItem;
use crate::crypto::TAG_LEN;

/// The base64 spelling of the largest sealed item this client accepts. No
/// column of a row this device could accept is longer.
pub const MAX_PAYLOAD_B64_BYTES: usize = match encoded_len(MAX_CONTENT_BYTES + TAG_LEN, true) {
    Some(len) => len,
    None => panic!("the payload bound overflows"),
};

/// Longest string that is never dropped for crowding a row: every bounded small
/// column, the longest being a 255-character bundle id, with every character
/// `\u00XX`-escaped.
const SMALL_STRING_BYTES: usize = 2 * 1024;

/// Room for 32 small strings with punctuation and numbers: every key and every
/// small value of a row, with margin.
const ROW_SLACK_BYTES: usize = 32 * SMALL_STRING_BYTES;

/// Bytes a row may reach before a longer-than-small string is dropped instead
/// of kept: the payload, metadata at its cap fully escaped, and the slack. An
/// honest row never reaches it.
const ROW_STRING_BUDGET: usize =
    MAX_PAYLOAD_B64_BYTES + 6 * MAX_SYNC_METADATA_BYTES + ROW_SLACK_BYTES;

/// The most one row may occupy. The second slack keeps room for the id, stamp
/// and other small columns that follow a dropped string.
pub const MAX_ROW_BYTES: usize = ROW_STRING_BUDGET + ROW_SLACK_BYTES;

/// Wire bytes of rows after which a page is cut at the next row boundary. One
/// maximal row is a whole page, and a page holds less than two.
pub const PAGE_BYTES: usize = MAX_ROW_BYTES;

/// One page of clipboard rows as the transport delivered it.
#[derive(Debug, Default)]
pub struct RowPage {
    /// In the order the server sent them.
    pub rows: Vec<CloudItem>,
    /// Rows with a string longer than [`MAX_PAYLOAD_B64_BYTES`], or long
    /// strings that together outgrow a row. Such strings were dropped unread,
    /// so these rows can only be refused.
    pub oversized: Vec<String>,
    /// The page was cut at [`PAGE_BYTES`]: more rows may follow even though
    /// fewer than the requested limit arrived.
    pub cut: bool,
}

pub(crate) enum BodyError {
    TooLarge,
    Network(reqwest::Error),
}

/// The whole body, refused as soon as it passes `cap`.
pub(crate) async fn read_capped(mut response: Response, cap: usize) -> Result<Vec<u8>, BodyError> {
    // Sized once from the declared length: growing by doubling would end a
    // full body at twice its size.
    let declared = response.content_length().unwrap_or(0);
    let mut body = Vec::with_capacity(declared.min(cap as u64) as usize);
    while let Some(chunk) = response.chunk().await.map_err(BodyError::Network)? {
        if body.len() + chunk.len() > cap {
            return Err(BodyError::TooLarge);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

/// At most the first `cap` bytes of the body, for a log line. A failed read
/// yields what arrived before it.
pub(crate) async fn read_prefix(mut response: Response, cap: usize) -> Vec<u8> {
    let mut body = Vec::new();
    while body.len() < cap {
        let Ok(Some(chunk)) = response.chunk().await else {
            break;
        };
        body.extend_from_slice(&chunk[..chunk.len().min(cap - body.len())]);
    }
    body
}

/// Read a JSON array of rows until it ends or reaches [`PAGE_BYTES`].
pub(crate) async fn read_row_page(mut response: Response) -> Result<RowPage, RestError> {
    let mut reader = RowPageReader::default();
    while let Some(chunk) = response.chunk().await.map_err(RestError::from_reqwest)? {
        if !reader.feed(&chunk)? {
            return reader.into_cut_page();
        }
    }
    reader.finish()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Start,
    First,
    Next,
    Row,
    After,
    End,
}

/// Splits a streamed JSON array of row objects, holding one row's bytes at a
/// time.
struct RowPageReader {
    page: RowPage,
    state: State,
    row: Vec<u8>,
    depth: usize,
    in_string: bool,
    escaped: bool,
    string_start: usize,
    string_len: usize,
    eliding: bool,
    elided: bool,
    page_bytes: usize,
}

impl Default for RowPageReader {
    fn default() -> Self {
        Self {
            page: RowPage::default(),
            state: State::Start,
            row: Vec::new(),
            depth: 0,
            in_string: false,
            escaped: false,
            string_start: 0,
            string_len: 0,
            eliding: false,
            elided: false,
            page_bytes: 0,
        }
    }
}

impl RowPageReader {
    /// Consume one chunk. `false` means the page is full and reading stops.
    fn feed(&mut self, chunk: &[u8]) -> Result<bool, RestError> {
        let mut at = 0;
        while at < chunk.len() {
            if self.state == State::Row {
                let used = self.scan_row(&chunk[at..])?;
                self.page_bytes = self.page_bytes.saturating_add(used);
                at += used;
                continue;
            }
            let byte = chunk[at];
            at += 1;
            if byte.is_ascii_whitespace() {
                continue;
            }
            self.state = match (self.state, byte) {
                (State::Start, b'[') => State::First,
                (State::First | State::After, b']') => State::End,
                (State::After, b',') if self.page_bytes >= PAGE_BYTES => return Ok(false),
                (State::After, b',') => State::Next,
                (State::First | State::Next, b'{') => {
                    self.row.push(b'{');
                    self.depth = 1;
                    self.page_bytes = self.page_bytes.saturating_add(1);
                    State::Row
                }
                _ => return Err(RestError::Malformed),
            };
        }
        Ok(true)
    }

    /// Scan row bytes until the row closes or the chunk ends; returns the
    /// number of bytes used.
    fn scan_row(&mut self, bytes: &[u8]) -> Result<usize, RestError> {
        let mut at = 0;
        while at < bytes.len() {
            if self.in_string {
                if self.escaped {
                    self.escaped = false;
                    self.string_bytes(&bytes[at..=at])?;
                    at += 1;
                    continue;
                }
                let run = bytes[at..]
                    .iter()
                    .position(|&b| b == b'"' || b == b'\\')
                    .unwrap_or(bytes.len() - at);
                self.string_bytes(&bytes[at..at + run])?;
                at += run;
                if let Some(&byte) = bytes.get(at) {
                    at += 1;
                    if byte == b'\\' {
                        self.escaped = true;
                        self.string_bytes(&[byte])?;
                    } else {
                        self.in_string = false;
                        self.eliding = false;
                        self.push(&[byte])?;
                    }
                }
                continue;
            }
            let run = bytes[at..]
                .iter()
                .position(|&b| matches!(b, b'"' | b'{' | b'}' | b'[' | b']'))
                .unwrap_or(bytes.len() - at);
            self.push(&bytes[at..at + run])?;
            at += run;
            let Some(&byte) = bytes.get(at) else { break };
            at += 1;
            self.push(&[byte])?;
            match byte {
                b'"' => {
                    self.in_string = true;
                    self.string_start = self.row.len();
                    self.string_len = 0;
                }
                b'{' | b'[' => self.depth += 1,
                _ => {
                    self.depth -= 1;
                    if self.depth == 0 {
                        self.emit()?;
                        self.state = State::After;
                        break;
                    }
                }
            }
        }
        Ok(at)
    }

    /// String content: kept until it outgrows any column this client accepts,
    /// or, once longer than a small column, would crowd the row past its string
    /// budget; then dropped, leaving an empty string in its place.
    fn string_bytes(&mut self, bytes: &[u8]) -> Result<(), RestError> {
        self.string_len = self.string_len.saturating_add(bytes.len());
        if self.eliding {
            return Ok(());
        }
        let crowds_row = self.string_len > SMALL_STRING_BYTES
            && self.row.len() + bytes.len() > ROW_STRING_BUDGET;
        if self.string_len > MAX_PAYLOAD_B64_BYTES || crowds_row {
            self.row.truncate(self.string_start);
            self.eliding = true;
            self.elided = true;
            return Ok(());
        }
        self.push(bytes)
    }

    fn push(&mut self, bytes: &[u8]) -> Result<(), RestError> {
        if self.row.len() + bytes.len() > MAX_ROW_BYTES {
            return Err(RestError::Malformed);
        }
        self.row.extend_from_slice(bytes);
        Ok(())
    }

    fn emit(&mut self) -> Result<(), RestError> {
        let item: CloudItem = serde_json::from_slice(&self.row).map_err(|_| {
            tracing::warn!("a row in the page was not the expected JSON object");
            RestError::Malformed
        })?;
        if self.elided {
            self.page.oversized.push(item.item_id.clone());
        }
        self.page.rows.push(item);
        self.row.clear();
        self.elided = false;
        Ok(())
    }

    fn finish(self) -> Result<RowPage, RestError> {
        if self.state != State::End {
            tracing::warn!("the row page was not the expected JSON array");
            return Err(RestError::Malformed);
        }
        Ok(self.page)
    }

    /// The rows read are a prefix of the server's order. That order compares
    /// `item_id` by the database collation, not by bytes, so no order check
    /// belongs here: a keyset bound past the byte-order maximum re-returns
    /// whatever collated before it, which costs a re-download and skips nothing.
    fn into_cut_page(mut self) -> Result<RowPage, RestError> {
        self.page.cut = true;
        Ok(self.page)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: &str, created_at: i64, ciphertext_len: usize) -> String {
        format!(
            r#"{{"item_id":"{id}","ciphertext":"{}","nonce":"bm9uY2U=","content_type":"text","created_at":{created_at},"deleted":false,"origin_device_id":"device-b","signature":"c2ln"}}"#,
            "A".repeat(ciphertext_len)
        )
    }

    /// Feed `body` in small chunks, as a socket would deliver it.
    fn read(body: &str) -> (Result<RowPage, RestError>, usize) {
        let mut reader = RowPageReader::default();
        let mut consumed = 0;
        for chunk in body.as_bytes().chunks(64 * 1024) {
            consumed += chunk.len();
            match reader.feed(chunk) {
                Ok(true) => {}
                Ok(false) => return (reader.into_cut_page(), consumed),
                Err(error) => return (Err(error), consumed),
            }
        }
        (reader.finish(), consumed)
    }

    #[test]
    fn a_large_page_is_cut_at_a_row_boundary_and_an_oversized_row_is_never_held() {
        let quarter = MAX_PAYLOAD_B64_BYTES / 4;
        let rows: Vec<String> = (0..12)
            .map(|i| row(&format!("item-{i:02}"), 1_000 + i, quarter))
            .collect();
        let body = format!("[{}]", rows.join(","));

        let (page, consumed) = read(&body);
        let page = page.unwrap();
        assert!(page.cut);
        assert_eq!(
            page.rows.len(),
            5,
            "the page stops at the first boundary past its budget"
        );
        assert!(
            consumed < body.len() / 2,
            "read {consumed} of {} bytes",
            body.len()
        );

        let hostile = 3 * MAX_PAYLOAD_B64_BYTES;
        let body = format!(
            "[{},{}]",
            row("huge", 1_000, hostile),
            row("next", 2_000, 8)
        );
        let mut reader = RowPageReader::default();
        let mut largest = 0;
        for chunk in body.as_bytes().chunks(64 * 1024) {
            if !reader.feed(chunk).unwrap() {
                break;
            }
            largest = largest.max(reader.row.capacity());
        }
        assert!(
            largest <= 2 * MAX_ROW_BYTES,
            "held {largest} bytes for one row"
        );
        let page = reader.into_cut_page().unwrap();
        assert_eq!(page.oversized, ["huge"]);
        assert!(page.rows[0].ciphertext.is_empty());
        assert_eq!(page.rows[0].created_at, 1_000);
    }

    #[test]
    fn long_strings_that_crowd_a_row_are_dropped_and_the_id_after_them_survives() {
        let body = format!(
            r#"[{{"ciphertext":"{}","nonce":"{}","content_type":"text","created_at":7,"deleted":false,"origin_device_id":"d","signature":"s","item_id":"poison"}},{}]"#,
            "A".repeat(MAX_PAYLOAD_B64_BYTES - 16),
            "B".repeat(2 * ROW_SLACK_BYTES + 6 * MAX_SYNC_METADATA_BYTES),
            row("next", 8, 4)
        );
        let page = read(&body).0.unwrap();
        assert_eq!(page.oversized, ["poison"]);
        assert_eq!(page.rows[0].created_at, 7);
        assert!(page.cut, "the poison row's wire bytes fill the page");
    }

    #[test]
    fn a_cut_page_keeps_the_server_collation_order() {
        let long = MAX_PAYLOAD_B64_BYTES * 3 / 5;
        let body = format!(
            "[{},{},{}]",
            row("a", 1, long),
            row("B", 1, long),
            row("c", 1, 4)
        );
        let page = read(&body).0.unwrap();
        assert!(page.cut);
        let ids: Vec<&str> = page.rows.iter().map(|r| r.item_id.as_str()).collect();
        assert_eq!(ids, ["a", "B"]);
    }

    #[test]
    fn every_split_point_reads_like_a_whole_body_parse() {
        let body = r#" [{"ciphertext":"q\"u\\o{t}e[]","nonce":"\u00e9\ud83d\ude00","content_type":"text/é漢","payload_metadata":"{\"name\":\"a}b\"}","created_at":1,"deleted":false,"origin_device_id":"d-1","signature":"s","item_id":"after-strings"},
            {"item_id":"b","ciphertext":"","nonce":"","content_type":"x","created_at":2,"deleted":true,"origin_device_id":"漢","signature":null}] "#;
        let whole: Vec<CloudItem> = serde_json::from_str(body).unwrap();
        for split in 0..=body.len() {
            let mut reader = RowPageReader::default();
            assert!(reader.feed(&body.as_bytes()[..split]).unwrap());
            assert!(reader.feed(&body.as_bytes()[split..]).unwrap());
            let page = reader.finish().unwrap();
            assert_eq!(page.rows, whole, "split at {split}");
            assert!(page.oversized.is_empty());
        }
    }

    #[test]
    fn a_short_page_reads_whole_and_a_broken_one_is_malformed() {
        let body = format!(" [ {} , {} ] ", row("a", 1, 4), row("b", 2, 4));
        let page = read(&body).0.unwrap();
        assert!(!page.cut);
        assert_eq!(page.rows.len(), 2);
        assert_eq!(read("[]").0.unwrap().rows.len(), 0);
        for broken in [
            r#"{"message":"nope"}"#,
            "[",
            r#"[{"item_id":"a"}]"#,
            "[1]",
            "[] x",
        ] {
            assert!(
                matches!(read(broken).0, Err(RestError::Malformed)),
                "{broken}"
            );
        }
    }
}
