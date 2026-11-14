//! Structure transforms: JSON, YAML, CSV and XML.
//!
//! JSON goes through an in-crate strict RFC 8259 reader that keeps numbers,
//! strings and key order exactly as written. YAML and XML are parsed without
//! resolving external entities, includes or anything else outside the text.

mod json;
mod table;
mod xml;
mod yaml;

use crate::catalogue::Catalogue;
use crate::params::{ParamSpec, Params};

pub(crate) fn register(catalogue: &mut Catalogue) {
    for def in [
        &json::FORMAT,
        &json::MINIFY,
        &json::VALIDATE,
        &json::SORT_KEYS,
        &json::QUOTE,
        &json::UNQUOTE,
        &yaml::TO_JSON,
        &yaml::FROM_JSON,
        &table::CSV_TO_JSON,
        &xml::FORMAT,
        &xml::MINIFY,
    ] {
        catalogue.add(def);
    }
}

/// The indentation choice shared by the formatting transforms.
const INDENT: ParamSpec = ParamSpec::choice("indent", &["2", "4", "tab"], "2");

fn indent_unit(params: &Params) -> &'static str {
    match params.choice("indent") {
        "4" => "    ",
        "tab" => "\t",
        _ => "  ",
    }
}
