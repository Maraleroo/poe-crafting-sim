//! The legal texts written by the PO, converted to HTML.
//!
//! The Markdown files in `web/legal/` are embedded at compile time and are never edited
//! by the code. Only the markup changes: the files become HTML when the page is built.
//! Two conversion rules keep the pages safe and the layout of the texts intact:
//! - a soft line break becomes `<br>`, so address blocks keep their lines;
//! - raw HTML in the Markdown becomes escaped text, so a legal file cannot embed content;
//! - an e-mail autolink stays plain text, so the pages never contain an e-mail link.

use pulldown_cmark::{html, Event, LinkType, Options, Parser, Tag, TagEnd};

const PRIVACY_DE: &str = include_str!("../legal/privacy.de.md");
const PRIVACY_EN: &str = include_str!("../legal/privacy.en.md");

/// HTML of the English privacy policy (a translation).
pub fn privacy_english_html() -> String {
    markdown_to_html(PRIVACY_EN)
}

/// HTML of the German privacy policy (the legally binding version).
pub fn privacy_german_html() -> String {
    markdown_to_html(PRIVACY_DE)
}

fn markdown_to_html(markdown: &str) -> String {
    let mut in_email_link = false;
    let events = Parser::new_ext(markdown, Options::empty()).filter_map(|event| match event {
        Event::Start(Tag::Link {
            link_type: LinkType::Email,
            ..
        }) => {
            in_email_link = true;
            None
        }
        Event::End(TagEnd::Link) if in_email_link => {
            in_email_link = false;
            None
        }
        Event::SoftBreak => Some(Event::HardBreak),
        Event::Html(raw) | Event::InlineHtml(raw) => Some(Event::Text(raw)),
        other => Some(other),
    });
    let mut output = String::new();
    html::push_html(&mut output, events);
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The text a visitor sees: tags removed, entities decoded, whitespace collapsed.
    fn visible_text(html: &str) -> String {
        let mut text = String::new();
        let mut in_tag = false;
        for c in html.chars() {
            match c {
                '<' => in_tag = true,
                '>' => in_tag = false,
                _ if !in_tag => text.push(c),
                _ => {}
            }
        }
        let text = text
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&amp;", "&");
        collapse_whitespace(&text)
    }

    /// The Markdown text without the heading, quote and list markers at line starts.
    fn text_without_markers(markdown: &str) -> String {
        let lines: Vec<&str> = markdown
            .lines()
            .map(|line| line.trim_start_matches(['#', '>', '-', ' ']))
            .collect();
        collapse_whitespace(&lines.join("\n"))
    }

    fn collapse_whitespace(text: &str) -> String {
        text.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    #[test]
    fn english_policy_keeps_the_wording_of_the_po_file() {
        assert_eq!(
            visible_text(&privacy_english_html()),
            text_without_markers(PRIVACY_EN)
        );
    }

    #[test]
    fn german_policy_keeps_the_wording_of_the_po_file() {
        assert_eq!(
            visible_text(&privacy_german_html()),
            text_without_markers(PRIVACY_DE)
        );
    }

    #[test]
    fn policies_have_headings_and_lists() {
        for html in [privacy_english_html(), privacy_german_html()] {
            assert!(html.contains("<h1>"));
            assert!(html.contains("<h2>"));
            assert!(html.contains("<ul>"));
        }
    }

    #[test]
    fn soft_line_breaks_become_line_breaks() {
        assert_eq!(markdown_to_html("one\ntwo"), "<p>one<br />\ntwo</p>\n");
    }

    #[test]
    fn raw_html_is_shown_as_text_and_never_embedded() {
        let html = markdown_to_html("a <b>bold</b> word\n\n<script>run()</script>\n");
        assert!(!html.contains("<b>"));
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;b&gt;bold&lt;/b&gt;"));
        assert!(html.contains("&lt;script&gt;run()&lt;/script&gt;"));
    }

    #[test]
    fn e_mail_autolinks_stay_plain_text() {
        let html = markdown_to_html("write to <a@b.c> today");
        assert!(!html.contains("<a "));
        assert!(html.contains("write to a@b.c today"));
    }

    #[test]
    fn policies_embed_no_content() {
        for html in [privacy_english_html(), privacy_german_html()] {
            assert!(!html.contains("<img"));
            assert!(!html.contains("<iframe"));
        }
    }
}
