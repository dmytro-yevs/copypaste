//! Developer transforms: JWT, timestamps, find and replace, extraction.

mod find;
mod jwt;
mod time;

use crate::catalogue::{Catalogue, Category, Context, Flags, Output, TransformDef};
use crate::error::TransformError;
use crate::params::{ParamSpec, Params};

pub(crate) fn register(catalogue: &mut Catalogue) {
    catalogue.add(&JWT_DECODE);
    catalogue.add(&TIMESTAMP_TO_DATE);
    catalogue.add(&DATE_TO_TIMESTAMP);
    catalogue.add(&FIND_REPLACE);
    catalogue.add(&EXTRACT);
    catalogue.add(&REMOVE_CHARS);
}

static TIMESTAMP_PARAMS: [ParamSpec; 2] = [
    ParamSpec::choice(
        "unit",
        &[
            "auto",
            "seconds",
            "milliseconds",
            "microseconds",
            "nanoseconds",
        ],
        "auto",
    ),
    ParamSpec::choice("format", &["iso_utc", "iso_local", "rfc2822"], "iso_utc"),
];

static DATE_PARAMS: [ParamSpec; 1] = [ParamSpec::choice(
    "unit",
    &["seconds", "milliseconds"],
    "seconds",
)];

static FIND_PARAMS: [ParamSpec; 6] = [
    ParamSpec::text("find", ""),
    ParamSpec::text("replace", ""),
    ParamSpec::flag("regex", false),
    ParamSpec::flag("case_sensitive", false),
    ParamSpec::flag("whole_word", false),
    ParamSpec::flag("multiline", true),
];

static EXTRACT_PARAMS: [ParamSpec; 3] = [
    ParamSpec::choice(
        "preset",
        &[
            "urls",
            "emails",
            "numbers",
            "ipv4",
            "ipv6",
            "hex_colors",
            "uuids",
            "custom",
        ],
        "urls",
    ),
    ParamSpec::text("pattern", ""),
    ParamSpec::flag("dedupe", false),
];

static REMOVE_PARAMS: [ParamSpec; 2] = [
    ParamSpec::choice(
        "class",
        &["digits", "punctuation", "non_ascii", "emoji", "custom"],
        "digits",
    ),
    ParamSpec::text("chars", ""),
];

static JWT_DECODE: TransformDef = TransformDef {
    id: "dev.jwt_decode",
    version: 1,
    category: Category::Dev,
    params: &[],
    flags: Flags {
        can_be_default: false,
        always_sensitive: true,
        ..Flags::PLAIN
    },
    modifier: None,
    example: (
        "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.c2ln",
        "{\n  \"header\": {\n    \"alg\": \"HS256\"\n  },\n  \"payload\": {\n    \"sub\": \"1\"\n  }\n}",
    ),
    run: run_jwt_decode,
};

static TIMESTAMP_TO_DATE: TransformDef = TransformDef {
    id: "dev.timestamp_to_date",
    version: 1,
    category: Category::Dev,
    params: &TIMESTAMP_PARAMS,
    flags: Flags::PLAIN,
    modifier: None,
    example: ("1700000000", "2023-11-14T22:13:20Z"),
    run: run_timestamp_to_date,
};

static DATE_TO_TIMESTAMP: TransformDef = TransformDef {
    id: "dev.date_to_timestamp",
    version: 1,
    category: Category::Dev,
    params: &DATE_PARAMS,
    flags: Flags::PLAIN,
    modifier: None,
    example: ("2023-11-14T22:13:20Z", "1700000000"),
    run: run_date_to_timestamp,
};

static FIND_REPLACE: TransformDef = TransformDef {
    id: "dev.find_replace",
    version: 1,
    category: Category::Dev,
    params: &FIND_PARAMS,
    flags: Flags {
        needs_free_text: true,
        ..Flags::PLAIN
    },
    modifier: None,
    example: ("colour, Colour", "colour, Colour"),
    run: run_find_replace,
};

static EXTRACT: TransformDef = TransformDef {
    id: "dev.extract",
    version: 1,
    category: Category::Dev,
    params: &EXTRACT_PARAMS,
    flags: Flags::PLAIN,
    modifier: None,
    example: (
        "Docs: https://x.test/a, mirror https://y.test.",
        "https://x.test/a\nhttps://y.test",
    ),
    run: run_extract,
};

static REMOVE_CHARS: TransformDef = TransformDef {
    id: "dev.remove_chars",
    version: 1,
    category: Category::Dev,
    params: &REMOVE_PARAMS,
    flags: Flags::PLAIN,
    modifier: None,
    example: ("R2-D2 v1.0", "R-D v."),
    run: run_remove_chars,
};

fn run_jwt_decode(input: &str, _: &Params, ctx: &Context) -> Result<Output, TransformError> {
    jwt::decode(input, ctx)
}

fn run_timestamp_to_date(
    input: &str,
    params: &Params,
    ctx: &Context,
) -> Result<Output, TransformError> {
    let unit = time::Unit::from_param(params.choice("unit"));
    time::timestamp_to_date(input, unit, params.choice("format"), ctx)
}

fn run_date_to_timestamp(
    input: &str,
    params: &Params,
    ctx: &Context,
) -> Result<Output, TransformError> {
    time::date_to_timestamp(input, params.choice("unit") == "milliseconds", ctx)
}

fn run_find_replace(input: &str, params: &Params, ctx: &Context) -> Result<Output, TransformError> {
    let pattern = find::Pattern {
        text: params.text("find"),
        literal: !params.flag("regex"),
        case_sensitive: params.flag("case_sensitive"),
        whole_word: params.flag("whole_word"),
        multiline: params.flag("multiline"),
    };
    find::replace(input, &pattern, params.text("replace"), ctx)
}

fn run_extract(input: &str, params: &Params, ctx: &Context) -> Result<Output, TransformError> {
    find::extract(
        input,
        params.choice("preset"),
        params.text("pattern"),
        params.flag("dedupe"),
        ctx,
    )
}

fn run_remove_chars(input: &str, params: &Params, ctx: &Context) -> Result<Output, TransformError> {
    find::remove(input, params.choice("class"), params.text("chars"), ctx)
}
