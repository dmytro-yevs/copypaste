//! On-demand rich-text conversion shared by every native clipboard host.

use crate::history_error::HistoryError;

pub(crate) fn plain_text(content_type: &str, content: &str) -> Result<String, HistoryError> {
    let text = match content_type {
        copypaste_ipc::content_type::HTML => {
            let rendered =
                html2text::config::with_decorator(html2text::render::TrivialDecorator::new())
                    .raw_mode(true)
                    .unicode_strikeout(false)
                    .string_from_read(content.as_bytes(), content.len().max(1))
                    .map_err(|_| HistoryError::InvalidRichText)?;
            // The renderer terminates its final line; that is not clipboard content.
            rendered.strip_suffix('\n').unwrap_or(&rendered).to_owned()
        }
        copypaste_ipc::content_type::RICH_TEXT => {
            let parser = rclip_rtf::Parser::new(content.as_bytes())
                .map_err(|_| HistoryError::InvalidRichText)?;
            let mut text = String::new();
            for run in parser {
                let run = run.map_err(|_| HistoryError::InvalidRichText)?;
                match run.text {
                    rclip_rtf::RunText::Text(value) => text.push_str(value),
                    rclip_rtf::RunText::Char(value) => text.push(value),
                    rclip_rtf::RunText::ParagraphBreak | rclip_rtf::RunText::LineBreak => {
                        text.push('\n');
                    }
                }
                if text.len() > copypaste_ipc::MAX_CONTENT_BYTES {
                    return Err(HistoryError::ContentTooLarge);
                }
            }
            text
        }
        _ => content.to_owned(),
    };
    if text.len() > copypaste_ipc::MAX_CONTENT_BYTES {
        return Err(HistoryError::ContentTooLarge);
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_removes_formatting_and_decodes_entities_without_wrapping_or_footnotes() {
        let input = "<style>hidden{}</style><p><b>Hello &amp; world</b></p><p><a href='https://example.com'>Привіт</a><br>next</p><script>hidden()</script>";
        assert_eq!(
            plain_text("text/html", input).unwrap(),
            "Hello & world\n\nПривіт\nnext"
        );
        let long = "word ".repeat(100);
        assert_eq!(
            plain_text("text/html", &format!("<p>{long}</p>")).unwrap(),
            long.trim_end()
        );
    }

    #[test]
    fn rtf_extracts_unicode_and_structural_whitespace_and_refuses_damaged_input() {
        assert_eq!(
            plain_text(
                "text/rtf",
                r"{\rtf1\ansi Hello \b world\b0\par next\tab \u1046?}"
            )
            .unwrap(),
            "Hello world\nnext\tЖ"
        );
        assert!(plain_text("text/rtf", "not an RTF document").is_err());
        assert_eq!(
            plain_text(
                "text/rtf",
                r"{\rtf1\ansi\ansicpg1251 \'cf\'f0\'e8\'e2\'b3\'f2}"
            )
            .unwrap(),
            "Привіт"
        );
        assert_eq!(
            plain_text("text/rtf", r"{\rtf1\ansi\uc1\u-10179?\u-8704?}").unwrap(),
            "😀"
        );
        assert_eq!(
            plain_text("text", "<b>literal code</b>\n").unwrap(),
            "<b>literal code</b>\n"
        );
    }
}
