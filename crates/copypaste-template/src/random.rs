//! `uuid` and `random`: a fresh value for every occurrence.

use rand::Rng;

/// A random version 4 UUID in its hyphenated lowercase form.
pub(crate) fn uuid() -> String {
    uuid::Uuid::new_v4().hyphenated().to_string()
}

/// A uniform integer in `min..=max`; the parser guarantees `min <= max`.
pub(crate) fn integer(min: i64, max: i64) -> String {
    rand::thread_rng().gen_range(min..=max).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_stay_in_range_and_shape() {
        assert_eq!(integer(7, 7), "7");
        assert!(integer(i64::MIN, i64::MAX).parse::<i64>().is_ok());
        let uuid = uuid();
        assert_eq!(uuid.len(), 36);
        assert_eq!(uuid.as_bytes()[14], b'4');
    }
}
