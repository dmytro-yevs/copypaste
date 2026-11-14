//! URL transforms: tracking-parameter removal and offline redirect unwrapping.

mod redirect;
pub(crate) mod scan;
pub mod tracking;

use crate::catalogue::{Catalogue, Category, Context, Flags, Output, TransformDef};
use crate::error::TransformError;
use crate::params::Params;
use tracking::{clean_urls, TrackingRules};

pub(crate) fn register(catalogue: &mut Catalogue) {
    catalogue.add(&REMOVE_TRACKING);
    catalogue.add(&UNWRAP_REDIRECTS);
}

static REMOVE_TRACKING: TransformDef = TransformDef {
    id: "url.remove_tracking",
    version: 1,
    category: Category::Url,
    params: &[],
    flags: Flags::PLAIN,
    modifier: None,
    example: (
        "https://x.test/p?id=7&utm_source=a&fbclid=b#top",
        "https://x.test/p?id=7#top",
    ),
    run: run_remove_tracking,
};

static UNWRAP_REDIRECTS: TransformDef = TransformDef {
    id: "url.unwrap_redirects",
    version: 1,
    category: Category::Url,
    params: &[],
    flags: Flags::PLAIN,
    modifier: None,
    example: (
        "https://www.google.com/url?q=https%3A%2F%2Fx.test%2Fp&sa=D",
        "https://x.test/p",
    ),
    run: run_unwrap_redirects,
};

/// Uses the user's lists from the context, or the built-in list alone.
fn run_remove_tracking(input: &str, _: &Params, ctx: &Context) -> Result<Output, TransformError> {
    match ctx.tracking() {
        Some(rules) => clean_urls(input, rules, ctx),
        None => clean_urls(input, &TrackingRules::default(), ctx),
    }
}

fn run_unwrap_redirects(input: &str, _: &Params, ctx: &Context) -> Result<Output, TransformError> {
    scan::rewrite_urls(input, ctx, |url, _| redirect::unwrap(url)).map(Output::new)
}
