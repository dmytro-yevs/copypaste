//! `date`, `time` and `datetime`: zone arithmetic with jiff, CLDR formatting
//! with ICU4X compiled data. OS formatters are never used, so one template
//! gives the same bytes on every platform for one locale, zone and instant.

use icu_calendar::{Date, Gregorian};
use icu_datetime::fieldsets::builder::{DateFields, FieldSetBuilder, ZoneStyle};
use icu_datetime::fieldsets::enums::CompositeFieldSet;
use icu_datetime::input::{DateTime, Time, TimeZone as CldrZone, UtcOffset, ZonedDateTime};
use icu_datetime::options::{Length, TimePrecision};
use icu_datetime::pattern::{DateTimePattern, FixedCalendarDateTimeNames};
use icu_datetime::FixedCalendarDateTimeFormatter;
use icu_locale::{LanguageIdentifier, Locale, LocaleExpander, TransformResult};
use jiff::tz::TimeZone;
use jiff::{Span, Timestamp, Zoned};
use writeable::TryWriteable;

use crate::lint::Issue;
use crate::syntax::Params;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DateKind {
    Date,
    Time,
    DateTime,
}

#[derive(Debug)]
pub(crate) enum Style {
    Short,
    Medium,
    Long,
    Full,
    Iso,
    Pattern(DateTimePattern),
}

#[derive(Debug)]
pub(crate) struct DateSpec {
    kind: DateKind,
    style: Style,
    locale: Option<Locale>,
    zone: Option<TimeZone>,
    offset: Vec<Span>,
    /// Name and parameters as written; equal keys give equal output within
    /// one expansion, so the renderer formats each key once.
    pub key: String,
}

pub(crate) fn parse_spec(name: &str, params: &mut Params, key: String) -> Result<DateSpec, Issue> {
    let kind = match name {
        "date" => DateKind::Date,
        "time" => DateKind::Time,
        _ => DateKind::DateTime,
    };
    let style = match (params.take("style"), params.take("format")) {
        (Some(_), Some(_)) => return Err(Issue::ConflictingParameters),
        (None, None) => Style::Medium,
        (None, Some(pattern)) => Style::Pattern(
            DateTimePattern::try_from_pattern_str(&pattern).map_err(|_| Issue::InvalidPattern)?,
        ),
        (Some(style), None) => match style.as_str() {
            "short" => Style::Short,
            "medium" => Style::Medium,
            "long" => Style::Long,
            "full" => Style::Full,
            "iso" => Style::Iso,
            _ => {
                return Err(Issue::InvalidValue {
                    key: "style".into(),
                })
            }
        },
    };
    let locale = params
        .take("locale")
        .map(|tag| known_locale(&tag).ok_or(Issue::UnknownLocale))
        .transpose()?;
    let zone = params
        .take("tz")
        .map(|id| resolve_zone(&id).ok_or(Issue::UnknownTimeZone))
        .transpose()?;
    let offset = match params.take("offset") {
        Some(offset) => parse_offset(&offset).ok_or_else(|| Issue::InvalidValue {
            key: "offset".into(),
        })?,
        None => Vec::new(),
    };
    Ok(DateSpec {
        kind,
        style,
        locale,
        zone,
        offset,
        key,
    })
}

/// Parses a BCP 47 tag whose language CLDR knows.
pub(crate) fn known_locale(tag: &str) -> Option<Locale> {
    let locale = Locale::try_from_str(tag).ok()?;
    let mut language = LanguageIdentifier::from(locale.id.language);
    let known = !locale.id.language.is_unknown()
        && LocaleExpander::new_common().maximize(&mut language) == TransformResult::Modified;
    known.then_some(locale)
}

/// An IANA zone name or `utc`, looked up in the platform zone database.
pub(crate) fn resolve_zone(id: &str) -> Option<TimeZone> {
    if id.eq_ignore_ascii_case("utc") {
        return Some(TimeZone::UTC);
    }
    TimeZone::get(id).ok()
}

/// `+1mo-2d`: signed terms with units y, mo, w, d, h, min, s (any case).
fn parse_offset(text: &str) -> Option<Vec<Span>> {
    let mut terms = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        let negative = match rest.as_bytes()[0] {
            b'+' => false,
            b'-' => true,
            _ => return None,
        };
        rest = &rest[1..];
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        let amount: i64 = rest[..digits].parse().ok()?;
        rest = &rest[digits..];
        let letters = rest.bytes().take_while(u8::is_ascii_alphabetic).count();
        let unit = rest[..letters].to_ascii_lowercase();
        rest = &rest[letters..];
        let amount = if negative { -amount } else { amount };
        let span = Span::new();
        let span = match unit.as_str() {
            "y" => span.try_years(amount),
            "mo" => span.try_months(amount),
            "w" => span.try_weeks(amount),
            "d" => span.try_days(amount),
            "h" => span.try_hours(amount),
            "min" => span.try_minutes(amount),
            "s" => span.try_seconds(amount),
            _ => return None,
        };
        terms.push(span.ok()?);
    }
    (!terms.is_empty()).then_some(terms)
}

/// Formats one date placeholder. `local_zone` is the device zone the caller
/// passed; `app_locale` applies when the placeholder names no locale.
pub(crate) fn format(
    spec: &DateSpec,
    instant: Timestamp,
    local_zone: Option<&TimeZone>,
    app_locale: &Locale,
) -> Result<String, Issue> {
    let zone = spec
        .zone
        .as_ref()
        .or(local_zone)
        .ok_or(Issue::UnknownTimeZone)?;
    let mut zoned = instant.to_zoned(zone.clone());
    // Applied one by one: calendar units clamp the day of month at each step
    // and clock units move the absolute time, as jiff does for a `Zoned`.
    for span in &spec.offset {
        zoned = zoned
            .checked_add(*span)
            .map_err(|_| Issue::DateOutOfRange)?;
    }
    let locale = spec.locale.as_ref().unwrap_or(app_locale);
    match &spec.style {
        Style::Iso => Ok(zoned
            .strftime(match spec.kind {
                DateKind::Date => "%Y-%m-%d",
                DateKind::Time => "%H:%M:%S",
                DateKind::DateTime => "%Y-%m-%dT%H:%M:%S%:z",
            })
            .to_string()),
        style => format_cldr(spec.kind, style, &zoned, locale),
    }
}

/// Formats with CLDR data in the Gregorian calendar, so a locale whose
/// default calendar differs still gives the same output everywhere.
fn format_cldr(
    kind: DateKind,
    style: &Style,
    zoned: &Zoned,
    locale: &Locale,
) -> Result<String, Issue> {
    let date = Date::try_new_gregorian(
        i32::from(zoned.year()),
        zoned.month().unsigned_abs(),
        zoned.day().unsigned_abs(),
    )
    .map_err(|_| Issue::DateOutOfRange)?;
    let time = Time::try_new(
        zoned.hour().unsigned_abs(),
        zoned.minute().unsigned_abs(),
        zoned.second().unsigned_abs(),
        zoned.subsec_nanosecond().unsigned_abs(),
    )
    .map_err(|_| Issue::DateOutOfRange)?;
    let offset = UtcOffset::try_from_seconds(zoned.offset().seconds()).ok();
    let id = zoned
        .time_zone()
        .iana_name()
        .map_or(CldrZone::UNKNOWN, CldrZone::from_iana_id);
    let zone = id.with_offset(offset).at_date_time(DateTime { date, time });
    let input = ZonedDateTime { date, time, zone };
    if let Style::Pattern(pattern) = style {
        let mut names = pattern_names(locale)?;
        let formatter = names
            .include_for_pattern(pattern)
            .map_err(|_| Issue::InvalidPattern)?;
        return formatter
            .format(&input)
            .try_write_to_string()
            .map(|text| text.into_owned())
            .map_err(|_| Issue::InvalidPattern);
    }
    let field_set = preset_field_set(kind, style)?;
    let formatter = FixedCalendarDateTimeFormatter::<Gregorian, CompositeFieldSet>::try_new(
        locale.into(),
        field_set,
    )
    .map_err(|_| Issue::UnknownLocale)?;
    Ok(formatter.format(&input).to_string())
}

fn pattern_names(
    locale: &Locale,
) -> Result<FixedCalendarDateTimeNames<Gregorian, CompositeFieldSet>, Issue> {
    FixedCalendarDateTimeNames::try_new(locale.into()).map_err(|_| Issue::UnknownLocale)
}

/// Presets: short, medium and long lengths; `full` is long with the weekday.
/// Times show seconds from medium up and a zone name from long up.
fn preset_field_set(kind: DateKind, style: &Style) -> Result<CompositeFieldSet, Issue> {
    let mut builder = FieldSetBuilder::new();
    builder.length = Some(match style {
        Style::Short => Length::Short,
        Style::Medium => Length::Medium,
        _ => Length::Long,
    });
    let full = matches!(style, Style::Full);
    if kind != DateKind::Time {
        builder.date_fields = Some(if full {
            DateFields::YMDE
        } else {
            DateFields::YMD
        });
    }
    if kind != DateKind::Date {
        builder.time_precision = Some(if matches!(style, Style::Short) {
            TimePrecision::Minute
        } else {
            TimePrecision::Second
        });
        builder.zone_style = match style {
            Style::Long => Some(ZoneStyle::SpecificShort),
            Style::Full => Some(ZoneStyle::SpecificLong),
            _ => None,
        };
    }
    builder.build_composite().map_err(|_| Issue::InvalidPattern)
}

/// Checks that a pattern can be formatted in `locale`; ICU refuses some
/// patterns (for example a textual field repeated in two widths) only once
/// it loads their names.
pub(crate) fn check_pattern(spec: &DateSpec, app_locale: &Locale) -> Result<(), Issue> {
    let Style::Pattern(pattern) = &spec.style else {
        return Ok(());
    };
    let locale = spec.locale.as_ref().unwrap_or(app_locale);
    pattern_names(locale)?
        .include_for_pattern(pattern)
        .map(|_| ())
        .map_err(|_| Issue::InvalidPattern)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offsets_need_a_sign_and_a_known_unit() {
        assert_eq!(parse_offset("+1mo-2D").map(|t| t.len()), Some(2));
        assert_eq!(parse_offset("+1MIN").map(|t| t.len()), Some(1));
        assert!(parse_offset("1d").is_none());
        assert!(parse_offset("+1m").is_none());
        assert!(parse_offset("+d").is_none());
        assert!(parse_offset("").is_none());
    }

    #[test]
    fn locales_must_be_known() {
        assert!(known_locale("uk-UA").is_some());
        assert!(known_locale("zz").is_none());
        assert!(known_locale("not a tag").is_none());
    }
}
