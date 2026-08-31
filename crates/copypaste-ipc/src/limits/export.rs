//! Allocation of an export response inside one bounded IPC frame.

use std::io::Cursor;

use crate::{ExportData, ExportItem, Response, ResponseData, MAX_FRAME_BYTES};

/// Measures export items against the largest response envelope this protocol permits.
///
/// The reservation uses maximal response id and skip counters, plus the newline
/// framing byte. Actual values cannot be wider, so a successful allocation always
/// fits the daemon's and embedded backend's frame boundary.
pub struct ExportFrameBudget {
    scratch: Box<[u8]>,
    reserved_overhead: usize,
    item_bytes: usize,
    items: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportFrameBudgetExceeded;

impl std::fmt::Display for ExportFrameBudgetExceeded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the export response exceeds its frame budget")
    }
}

impl std::error::Error for ExportFrameBudgetExceeded {}

impl Default for ExportFrameBudget {
    fn default() -> Self {
        Self::new()
    }
}

impl ExportFrameBudget {
    #[must_use]
    pub fn new() -> Self {
        let mut scratch = vec![0; MAX_FRAME_BYTES].into_boxed_slice();
        let empty = ExportData {
            items: Vec::new(),
            skipped_non_text: u32::MAX,
            skipped_sensitive: u32::MAX,
            skipped_undecryptable: u32::MAX,
        };
        let mut cursor = Cursor::new(scratch.as_mut());
        serde_json::to_writer(
            &mut cursor,
            &Response::ok(u64::MAX, ResponseData::Export(empty)),
        )
        .expect("the maximal empty export response fits one frame");
        let empty_response_bytes = cursor.position() as usize + 1;

        Self {
            scratch,
            reserved_overhead: empty_response_bytes,
            item_bytes: 0,
            items: 0,
        }
    }

    /// Reserve room for one item without serializing the growing vector.
    ///
    /// A fixed cursor makes a serialization that cannot fit report `WriteZero`;
    /// this has one product meaning here: the export cannot fit its IPC reply.
    pub fn try_push(&mut self, item: &ExportItem) -> Result<(), ExportFrameBudgetExceeded> {
        let mut cursor = Cursor::new(self.scratch.as_mut());
        serde_json::to_writer(&mut cursor, item).map_err(|_| ExportFrameBudgetExceeded)?;
        let item_bytes = cursor.position() as usize;
        let comma = usize::from(self.items > 0);
        let used = self
            .reserved_overhead
            .checked_add(self.item_bytes)
            .and_then(|bytes| bytes.checked_add(comma))
            .and_then(|bytes| bytes.checked_add(item_bytes))
            .ok_or(ExportFrameBudgetExceeded)?;
        if used > MAX_FRAME_BYTES {
            return Err(ExportFrameBudgetExceeded);
        }
        self.item_bytes = self
            .item_bytes
            .checked_add(comma + item_bytes)
            .ok_or(ExportFrameBudgetExceeded)?;
        self.items += 1;
        Ok(())
    }

    #[cfg(test)]
    fn estimated_frame_bytes(&self) -> usize {
        self.reserved_overhead + self.item_bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(content: String) -> ExportItem {
        ExportItem {
            content,
            content_type: "text/plain".into(),
            created_at: i64::MAX,
            pinned: true,
            is_sensitive: true,
        }
    }

    fn response(items: Vec<ExportItem>) -> Response {
        Response::ok(
            u64::MAX,
            ResponseData::Export(ExportData {
                items,
                skipped_non_text: u32::MAX,
                skipped_sensitive: u32::MAX,
                skipped_undecryptable: u32::MAX,
            }),
        )
    }

    #[test]
    fn exact_budget_including_the_newline_accepts_and_one_byte_over_refuses() {
        let baseline = serde_json::to_vec(&response(vec![item(String::new())]))
            .unwrap()
            .len()
            + 1;
        let controls = (MAX_FRAME_BYTES - baseline) / 6;
        let remainder = (MAX_FRAME_BYTES - baseline) % 6;
        let mut content = "\u{1}".repeat(controls);
        content.push_str(&"a".repeat(remainder));
        let item = item(content);
        let mut budget = ExportFrameBudget::new();
        assert!(budget.try_push(&item).is_ok());

        let actual = serde_json::to_vec(&response(vec![item.clone()]))
            .unwrap()
            .len()
            + 1;
        assert_eq!(actual, MAX_FRAME_BYTES);
        assert_eq!(budget.estimated_frame_bytes(), actual);

        let mut over = item;
        over.content.push('a');
        let over_actual = serde_json::to_vec(&response(vec![over.clone()]))
            .unwrap()
            .len()
            + 1;
        assert_eq!(over_actual, MAX_FRAME_BYTES + 1);
        let mut budget = ExportFrameBudget::new();
        assert!(budget.try_push(&over).is_err());
    }

    #[test]
    fn estimate_matches_empty_and_comma_separated_max_width_responses() {
        let first = item("\u{1}".into());
        let second = item(String::from_utf8_lossy(&[0xff, b'a']).into_owned());
        let mut budget = ExportFrameBudget::new();

        assert_eq!(
            budget.estimated_frame_bytes(),
            serde_json::to_vec(&response(vec![])).unwrap().len() + 1
        );
        budget.try_push(&first).unwrap();
        assert_eq!(
            budget.estimated_frame_bytes(),
            serde_json::to_vec(&response(vec![first.clone()]))
                .unwrap()
                .len()
                + 1
        );
        budget.try_push(&second).unwrap();
        assert_eq!(
            budget.estimated_frame_bytes(),
            serde_json::to_vec(&response(vec![first, second]))
                .unwrap()
                .len()
                + 1
        );
    }

    #[test]
    fn reservation_covers_controls_lossy_text_and_real_response_values() {
        let item = item(String::from_utf8_lossy(&[0xff, 1, b'a']).into_owned());
        let mut budget = ExportFrameBudget::new();
        budget.try_push(&item).unwrap();
        let actual = serde_json::to_vec(&response(vec![item])).unwrap().len() + 1;
        assert!(actual <= MAX_FRAME_BYTES);
    }
}
