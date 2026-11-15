//! Cleanup transforms: invisible characters, curly quotes and dashes.

use unicode_segmentation::UnicodeSegmentation;

use crate::catalogue::{Catalogue, Category, Context, Flags, Output, TransformDef};
use crate::error::TransformError;
use crate::params::{ParamSpec, Params};

pub(crate) fn register(catalogue: &mut Catalogue) {
    catalogue.add(&INVISIBLE);
    catalogue.add(&QUOTES);
}

static QUOTES_PARAMS: [ParamSpec; 1] = [ParamSpec::flag("dashes", false)];

static INVISIBLE: TransformDef = TransformDef {
    id: "clean.invisible",
    version: 1,
    category: Category::Clean,
    params: &[],
    flags: Flags::PLAIN,
    modifier: None,
    example: ("in\u{200b}visi\u{ad}ble\u{a0}text", "invisible text"),
    run: run_invisible,
};

static QUOTES: TransformDef = TransformDef {
    id: "clean.quotes",
    version: 1,
    category: Category::Clean,
    params: &QUOTES_PARAMS,
    flags: Flags::PLAIN,
    modifier: None,
    example: ("\u{201c}It\u{2019}s\u{201d}", "\"It's\""),
    run: run_quotes,
};

const ZWJ: char = '\u{200d}';

/// Zero-width and format characters removed outright; ZWJ is handled
/// separately because emoji sequences need it.
fn is_invisible(c: char) -> bool {
    matches!(
        c,
        '\u{ad}'
            | '\u{61c}'
            | '\u{200b}'
            | '\u{200c}'
            | '\u{200e}'
            | '\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2060}'
            | '\u{2066}'..='\u{2069}'
            | '\u{feff}'
    )
}

/// Removes invisible characters per grapheme cluster. A ZWJ inside a
/// cluster joins an emoji sequence and stays; one that ends its cluster
/// joins nothing and goes. No-break spaces become plain spaces.
fn run_invisible(input: &str, _: &Params, ctx: &Context) -> Result<Output, TransformError> {
    let mut out = String::with_capacity(input.len());
    for (i, grapheme) in input.graphemes(true).enumerate() {
        ctx.tick(i)?;
        let last = grapheme.chars().count() - 1;
        for (n, c) in grapheme.chars().enumerate() {
            match c {
                ZWJ if n == last => {}
                '\u{a0}' | '\u{202f}' => out.push(' '),
                c if is_invisible(c) => {}
                c => out.push(c),
            }
        }
    }
    Ok(Output::new(out))
}

fn run_quotes(input: &str, params: &Params, ctx: &Context) -> Result<Output, TransformError> {
    let dashes = params.flag("dashes");
    let mut out = String::with_capacity(input.len());
    for (i, c) in input.chars().enumerate() {
        ctx.tick(i)?;
        out.push(match c {
            '\u{2018}' | '\u{2019}' | '\u{201a}' | '\u{201b}' => '\'',
            '\u{201c}' | '\u{201d}' | '\u{201e}' | '\u{201f}' => '"',
            '\u{2013}' | '\u{2014}' if dashes => '-',
            c => c,
        });
    }
    Ok(Output::new(out))
}
