//! Order keys for user-arranged lists.

const ORDER_DIGITS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

fn order_digit(byte: u8) -> usize {
    ORDER_DIGITS
        .iter()
        .position(|digit| *digit == byte)
        .unwrap_or(if byte < b'0' {
            0
        } else {
            ORDER_DIGITS.len() - 1
        })
}

/// A key strictly between `low` and `high` (fractional indexing over base-62
/// digits), so a move rewrites one record. `None` is the open end.
pub(crate) fn order_key_between(low: Option<&str>, high: Option<&str>) -> String {
    fn midpoint(low: &[usize], high: Option<&[usize]>, out: &mut Vec<usize>) {
        if let Some(high) = high {
            let shared = high
                .iter()
                .enumerate()
                .take_while(|(at, digit)| low.get(*at).copied().unwrap_or(0) == **digit)
                .count();
            if shared > 0 {
                out.extend_from_slice(&high[..shared]);
                return midpoint(low.get(shared..).unwrap_or(&[]), Some(&high[shared..]), out);
            }
        }
        let below = low.first().copied().unwrap_or(0);
        let above = high
            .and_then(|high| high.first().copied())
            .unwrap_or(ORDER_DIGITS.len());
        if above > below + 1 {
            out.push((below + above) / 2);
        } else if let Some(high) = high.filter(|high| high.len() > 1) {
            out.push(high[0]);
        } else {
            out.push(below);
            midpoint(low.get(1..).unwrap_or(&[]), None, out);
        }
    }
    let digits = |key: &str| key.bytes().map(order_digit).collect::<Vec<_>>();
    let low = low.map(digits).unwrap_or_default();
    let high = high.map(digits);
    let mut out = Vec::new();
    midpoint(&low, high.as_deref(), &mut out);
    out.into_iter()
        .map(|digit| ORDER_DIGITS[digit] as char)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_keys_always_fall_strictly_between_their_neighbours() {
        let low = order_key_between(None, None);
        let mut high = order_key_between(Some(&low), None);
        assert!(low < high);
        for _ in 0..200 {
            let key = order_key_between(Some(&low), Some(&high));
            assert!(low < key && key < high, "{low} {key} {high}");
            assert!(!key.ends_with('0') && key.len() <= 128);
            high = key;
        }
        let first = order_key_between(None, Some(&low));
        assert!(first < low && !first.ends_with('0'));
    }
}
