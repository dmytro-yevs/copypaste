//! Epoch timestamps to dates and back.

use jiff::civil::{Date, DateTime};
use jiff::fmt::rfc2822;
use jiff::tz::TimeZone;
use jiff::{Timestamp, Zoned};

use crate::catalogue::{Context, Output};
use crate::error::{TransformError, Warning};

/// Auto-detected values must land in 1970-01-01 ..= 2100-12-31 (UTC).
const LAST_AUTO_SECOND: i64 = 4_133_980_799;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Unit {
    Auto,
    Seconds,
    Milliseconds,
    Microseconds,
    Nanoseconds,
}

impl Unit {
    pub(super) fn from_param(value: &str) -> Self {
        match value {
            "seconds" => Self::Seconds,
            "milliseconds" => Self::Milliseconds,
            "microseconds" => Self::Microseconds,
            "nanoseconds" => Self::Nanoseconds,
            _ => Self::Auto,
        }
    }

    fn nanos(self) -> i128 {
        match self {
            Self::Auto | Self::Seconds => 1_000_000_000,
            Self::Milliseconds => 1_000_000,
            Self::Microseconds => 1_000,
            Self::Nanoseconds => 1,
        }
    }

    /// The unit implied by the number of integer digits.
    fn by_digits(digits: usize) -> Option<Self> {
        match digits {
            9 | 10 => Some(Self::Seconds),
            12 | 13 => Some(Self::Milliseconds),
            15 | 16 => Some(Self::Microseconds),
            18 | 19 => Some(Self::Nanoseconds),
            _ => None,
        }
    }
}

pub(super) fn timestamp_to_date(
    input: &str,
    unit: Unit,
    format: &str,
    ctx: &Context,
) -> Result<Output, TransformError> {
    let start = input.len() - input.trim_start().len();
    let text = input.trim();
    let (negative, integer, fraction) = split_number(text).map_err(|bad| {
        TransformError::invalid_input("timestamp.not_a_number", input, start + bad)
    })?;
    let unit_required = || TransformError::InvalidInput {
        code: "timestamp.unit_required",
        location: None,
    };
    let out_of_range = || TransformError::InvalidInput {
        code: "timestamp.out_of_range",
        location: None,
    };
    let auto = unit == Unit::Auto;
    let unit = match unit {
        Unit::Auto => Unit::by_digits(integer.len()).ok_or_else(unit_required)?,
        unit => unit,
    };
    let whole: i128 = integer.parse().map_err(|_| out_of_range())?;
    let digits = &fraction[..fraction.len().min(9)];
    let scale = 10i128.pow(u32::try_from(digits.len()).unwrap_or(0));
    let part = if digits.is_empty() {
        0
    } else {
        digits.parse::<i128>().unwrap_or(0) * unit.nanos() / scale
    };
    let mut nanos = whole
        .checked_mul(unit.nanos())
        .and_then(|n| n.checked_add(part))
        .ok_or_else(out_of_range)?;
    if negative {
        nanos = -nanos;
    }
    let timestamp = Timestamp::from_nanosecond(nanos).map_err(|_| out_of_range())?;
    if auto && !(0..=LAST_AUTO_SECOND).contains(&timestamp.as_second()) {
        return Err(unit_required());
    }
    let text = match format {
        "iso_local" => {
            let zone = ctx.time_zone().cloned().unwrap_or(TimeZone::UTC);
            timestamp
                .display_with_offset(zone.to_offset(timestamp))
                .to_string()
        }
        "rfc2822" => {
            rfc2822::to_string(&timestamp.to_zoned(TimeZone::UTC)).map_err(|_| out_of_range())?
        }
        _ => timestamp.to_string(),
    };
    Ok(Output::new(text))
}

/// Sign, integer digits and fraction digits, or the offset of the first
/// character that does not fit.
fn split_number(text: &str) -> Result<(bool, &str, &str), usize> {
    let (negative, body, sign) = match text.as_bytes().first() {
        Some(b'-') => (true, &text[1..], 1),
        Some(b'+') => (false, &text[1..], 1),
        _ => (false, text, 0),
    };
    let (integer, fraction) = body.split_once('.').unwrap_or((body, ""));
    let bad_integer = integer.bytes().position(|b| !b.is_ascii_digit());
    if let Some(at) = bad_integer {
        return Err(sign + at);
    }
    if integer.is_empty() {
        return Err(sign);
    }
    let fraction_start = sign + integer.len() + 1;
    match fraction.bytes().position(|b| !b.is_ascii_digit()) {
        Some(at) => Err(fraction_start + at),
        None if body.ends_with('.') => Err(fraction_start - 1),
        None => Ok((negative, integer, fraction)),
    }
}

pub(super) fn date_to_timestamp(
    input: &str,
    millis: bool,
    ctx: &Context,
) -> Result<Output, TransformError> {
    let start = input.len() - input.trim_start().len();
    let text = input.trim();
    let mut warnings = Vec::new();
    let timestamp = if let Ok(ts) = text.parse::<Timestamp>() {
        ts
    } else if let Ok(zoned) = text.parse::<Zoned>() {
        zoned.timestamp()
    } else if let Ok(zoned) = rfc2822::parse(text) {
        zoned.timestamp()
    } else {
        let civil = text
            .parse::<DateTime>()
            .or_else(|_| {
                text.parse::<Date>()
                    .map(|d| d.to_datetime(jiff::civil::Time::midnight()))
            })
            .map_err(|_| TransformError::invalid_input("date.unrecognized", input, start))?;
        let zone = ctx.time_zone().cloned().unwrap_or(TimeZone::UTC);
        warnings.push(Warning::new("date.local_zone_assumed"));
        zone.to_ambiguous_timestamp(civil)
            .compatible()
            .map_err(|_| TransformError::invalid_input("date.unrecognized", input, start))?
    };
    let value = if millis {
        timestamp.as_millisecond()
    } else {
        timestamp.as_second()
    };
    Ok(Output {
        text: value.to_string(),
        warnings,
        sensitive: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_formats_use_the_context_zone() {
        let zone = TimeZone::get("Europe/Kyiv").unwrap();
        let ctx = Context::new().with_time_zone(zone);
        assert_eq!(
            timestamp_to_date("1700000000", Unit::Auto, "iso_local", &ctx)
                .unwrap()
                .text,
            "2023-11-15T00:13:20+02:00"
        );
        let back = date_to_timestamp("2023-11-15 00:13:20", false, &ctx).unwrap();
        assert_eq!(back.text, "1700000000");
        assert_eq!(back.warnings, [Warning::new("date.local_zone_assumed")]);
    }

    #[test]
    fn malformed_numbers_point_at_the_bad_character() {
        let err = timestamp_to_date(" 17000x", Unit::Auto, "iso_utc", &Context::new()).unwrap_err();
        assert_eq!(
            err,
            TransformError::invalid_input("timestamp.not_a_number", " 17000x", 6)
        );
    }
}
