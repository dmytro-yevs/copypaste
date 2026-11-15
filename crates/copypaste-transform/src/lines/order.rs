//! Sorting, de-duplication and reversal of lines. Sorting is stable, and
//! case folding uses ICU4X simple folding so every platform orders alike.

use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::HashSet;

use icu_casemap::CaseMapper;
use unicode_segmentation::UnicodeSegmentation;

use super::Lines;
use crate::catalogue::{Category, Context, Flags, Output, TransformDef};
use crate::error::TransformError;
use crate::params::{ParamSpec, Params};

static SORT_PARAMS: [ParamSpec; 2] = [
    ParamSpec::choice("order", &["asc", "desc"], "asc"),
    ParamSpec::choice(
        "mode",
        &[
            "natural",
            "code_point",
            "case_insensitive",
            "numeric",
            "length",
        ],
        "natural",
    ),
];

static DEDUPE_PARAMS: [ParamSpec; 2] = [
    ParamSpec::flag("case_sensitive", true),
    ParamSpec::flag("compare_trimmed", false),
];

pub(super) static SORT: TransformDef = TransformDef {
    id: "lines.sort",
    version: 1,
    category: Category::Lines,
    params: &SORT_PARAMS,
    flags: Flags::PLAIN,
    modifier: None,
    example: ("file10\nfile2\nFile1", "File1\nfile2\nfile10"),
    run: run_sort,
};

pub(super) static DEDUPE: TransformDef = TransformDef {
    id: "lines.dedupe",
    version: 1,
    category: Category::Lines,
    params: &DEDUPE_PARAMS,
    flags: Flags::PLAIN,
    modifier: None,
    example: ("b\na\nb\nc\na", "b\na\nc"),
    run: run_dedupe,
};

pub(super) static REVERSE: TransformDef = TransformDef {
    id: "lines.reverse",
    version: 1,
    category: Category::Lines,
    params: &[],
    flags: Flags::PLAIN,
    modifier: None,
    example: ("one\ntwo\nthree", "three\ntwo\none"),
    run: run_reverse,
};

fn fold(c: char) -> char {
    CaseMapper::new().simple_fold(c)
}

fn case_insensitive(a: &str, b: &str) -> Ordering {
    a.chars().map(fold).cmp(b.chars().map(fold))
}

/// The ASCII digit run at the start of `s`, and the rest.
fn split_digits(s: &str) -> (&str, &str) {
    let end = s
        .bytes()
        .position(|b| !b.is_ascii_digit())
        .unwrap_or(s.len());
    s.split_at(end)
}

/// Digit runs compare by value, then by length so `01` follows `1`; other
/// characters compare case-insensitively.
fn natural(mut a: &str, mut b: &str) -> Ordering {
    loop {
        let (Some(x), Some(y)) = (a.chars().next(), b.chars().next()) else {
            return a.is_empty().cmp(&b.is_empty()).reverse();
        };
        if x.is_ascii_digit() && y.is_ascii_digit() {
            let (da, ra) = split_digits(a);
            let (db, rb) = split_digits(b);
            let (va, vb) = (da.trim_start_matches('0'), db.trim_start_matches('0'));
            let order = va
                .len()
                .cmp(&vb.len())
                .then_with(|| va.cmp(vb))
                .then_with(|| da.len().cmp(&db.len()));
            if order != Ordering::Equal {
                return order;
            }
            (a, b) = (ra, rb);
        } else {
            let order = fold(x).cmp(&fold(y));
            if order != Ordering::Equal {
                return order;
            }
            (a, b) = (&a[x.len_utf8()..], &b[y.len_utf8()..]);
        }
    }
}

/// The number a line starts with (`-12.5 apples` → -12.5), if any.
fn leading_number(line: &str) -> Option<f64> {
    let s = line.trim_start();
    let sign = usize::from(s.starts_with(['-', '+']));
    let (int, rest) = split_digits(&s[sign..]);
    let fraction = rest
        .strip_prefix('.')
        .map_or(0, |r| 1 + split_digits(r).0.len());
    if int.is_empty() && fraction <= 1 {
        return None;
    }
    let end = sign + int.len() + if fraction > 1 { fraction } else { 0 };
    s[..end].parse().ok()
}

fn numeric(a: &Option<f64>, b: &Option<f64>) -> Ordering {
    match (a, b) {
        (Some(x), Some(y)) => x.total_cmp(y),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn run_sort(input: &str, params: &Params, ctx: &Context) -> Result<Output, TransformError> {
    let lines = Lines::split(input);
    let line = |i: usize| lines.lines[i];
    let mode = params.choice("mode");
    let numbers: Vec<Option<f64>> = match mode {
        "numeric" => lines.lines.iter().map(|l| leading_number(l)).collect(),
        _ => Vec::new(),
    };
    let lengths: Vec<usize> = match mode {
        "length" => lines
            .lines
            .iter()
            .map(|l| l.graphemes(true).count())
            .collect(),
        _ => Vec::new(),
    };
    let compare = |a: usize, b: usize| match mode {
        "code_point" => line(a).cmp(line(b)),
        "case_insensitive" => case_insensitive(line(a), line(b)),
        "numeric" => numeric(&numbers[a], &numbers[b]),
        "length" => lengths[a].cmp(&lengths[b]),
        _ => natural(line(a), line(b)),
    };
    let descending = params.choice("order") == "desc";
    let mut order: Vec<usize> = (0..lines.lines.len()).collect();
    // A stable sort keeps equal lines in input order in both directions.
    order.sort_by(|&a, &b| {
        if descending {
            compare(b, a)
        } else {
            compare(a, b)
        }
    });
    ctx.check()?;
    let text = lines.join(order.into_iter().map(line), ctx)?;
    Ok(Output::new(text))
}

fn run_dedupe(input: &str, params: &Params, ctx: &Context) -> Result<Output, TransformError> {
    let case_sensitive = params.flag("case_sensitive");
    let trimmed = params.flag("compare_trimmed");
    let lines = Lines::split(input);
    let mut seen: HashSet<Cow<str>> = HashSet::new();
    let mut kept = Vec::new();
    for (i, &line) in lines.lines.iter().enumerate() {
        ctx.tick(i)?;
        let key = if trimmed { line.trim() } else { line };
        let key = if case_sensitive {
            Cow::Borrowed(key)
        } else {
            CaseMapper::new().fold_string(key)
        };
        if seen.insert(key) {
            kept.push(line);
        }
    }
    Ok(Output::new(lines.join(kept, ctx)?))
}

fn run_reverse(input: &str, _: &Params, ctx: &Context) -> Result<Output, TransformError> {
    let lines = Lines::split(input);
    let text = lines.join(lines.lines.iter().rev(), ctx)?;
    Ok(Output::new(text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn natural_order_compares_numbers_by_value() {
        assert_eq!(natural("file2", "file10"), Ordering::Less);
        assert_eq!(natural("File1", "file2"), Ordering::Less);
        assert_eq!(natural("a1", "a01"), Ordering::Less);
        assert_eq!(natural("a", "a1"), Ordering::Less);
    }

    #[test]
    fn leading_numbers_parse_signs_and_fractions() {
        assert_eq!(leading_number(" -12.5 apples"), Some(-12.5));
        assert_eq!(leading_number("7."), Some(7.0));
        assert_eq!(leading_number(".5"), Some(0.5));
        assert_eq!(leading_number("x1"), None);
        assert_eq!(leading_number("-"), None);
    }
}
