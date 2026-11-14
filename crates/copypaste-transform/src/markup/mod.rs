//! Markup transforms: rich text, HTML and Markdown to other forms.

mod html;
mod markdown;

use crate::catalogue::{Catalogue, Category, Context, Flags, Output, TransformDef};
use crate::error::TransformError;
use crate::params::{ParamSpec, Params};

pub(crate) fn register(catalogue: &mut Catalogue) {
    catalogue.add(&RICH_TO_PLAIN);
    catalogue.add(&HTML_TO_MARKDOWN);
    catalogue.add(&MARKDOWN_TO_PLAIN);
}

/// Reads the HTML flavor of a rich item.
const RICH: Flags = Flags {
    rich_input: true,
    ..Flags::PLAIN
};

static MARKDOWN_PARAMS: [ParamSpec; 2] = [
    ParamSpec::choice("link_style", &["text", "text_url"], "text_url"),
    ParamSpec::flag("keep_list_markers", true),
];

static RICH_TO_PLAIN: TransformDef = TransformDef {
    id: "markup.rich_to_plain",
    version: 1,
    category: Category::Markup,
    params: &[],
    flags: RICH,
    modifier: None,
    example: ("<p>Hello <b>world</b></p>", "Hello world"),
    run: run_rich_to_plain,
};

static HTML_TO_MARKDOWN: TransformDef = TransformDef {
    id: "markup.html_to_markdown",
    version: 1,
    category: Category::Markup,
    params: &[],
    flags: RICH,
    modifier: None,
    example: (
        "<h2>Notes</h2><p>See <a href=\"https://x.test\">this</a></p>",
        "## Notes\n\nSee [this](https://x.test)",
    ),
    run: run_html_to_markdown,
};

static MARKDOWN_TO_PLAIN: TransformDef = TransformDef {
    id: "markup.markdown_to_plain",
    version: 1,
    category: Category::Markup,
    params: &MARKDOWN_PARAMS,
    flags: Flags::PLAIN,
    modifier: None,
    example: (
        "## Notes\n**See** [this](https://x.test)",
        "Notes\nSee this (https://x.test)",
    ),
    run: run_markdown_to_plain,
};

fn run_rich_to_plain(input: &str, _: &Params, ctx: &Context) -> Result<Output, TransformError> {
    html::to_plain(input, ctx).map(Output::new)
}

fn run_html_to_markdown(input: &str, _: &Params, ctx: &Context) -> Result<Output, TransformError> {
    html::to_markdown(input, ctx).map(Output::new)
}

fn run_markdown_to_plain(
    input: &str,
    params: &Params,
    ctx: &Context,
) -> Result<Output, TransformError> {
    let options = markdown::Options {
        link_urls: params.choice("link_style") == "text_url",
        list_markers: params.flag("keep_list_markers"),
    };
    markdown::to_plain(input, options, ctx).map(Output::new)
}
