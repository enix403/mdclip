//! Markdown -> HTML rendering.
//!
//! A single pure function, [`render`], that turns markdown into the bare
//! semantic HTML we place on the clipboard. GFM extensions (tables,
//! strikethrough, autolinks, task lists) are explicitly enabled — without
//! them `comrak` falls back to plain CommonMark and tables render as literal
//! pipes, defeating the reason `comrak` was chosen.

use comrak::Options;

/// Render `markdown` into bare semantic HTML with GFM extensions enabled.
///
/// Pure: same input always yields the same output. No inline CSS, no syntax
/// highlighting — paste targets re-style semantic HTML with their own theme.
pub fn render(markdown: &str) -> String {
    let mut options = Options::default();
    options.extension.table = true;
    options.extension.strikethrough = true;
    options.extension.autolink = true;
    options.extension.tasklist = true;
    comrak::markdown_to_html(markdown, &options)
}

#[cfg(test)]
mod tests {
    use super::render;

    #[test]
    fn heading() {
        assert_eq!(render("# Hello\n\nworld"), "<h1>Hello</h1>\n<p>world</p>\n");
    }

    #[test]
    fn heading_level_two() {
        assert_eq!(render("## Section"), "<h2>Section</h2>\n");
    }

    #[test]
    fn bold() {
        assert_eq!(render("**bold**"), "<p><strong>bold</strong></p>\n");
    }

    #[test]
    fn italic() {
        assert_eq!(render("*italic*"), "<p><em>italic</em></p>\n");
    }

    #[test]
    fn inline_code() {
        assert_eq!(render("`code`"), "<p><code>code</code></p>\n");
    }

    #[test]
    fn unordered_list() {
        assert_eq!(
            render("- a\n- b"),
            "<ul>\n<li>a</li>\n<li>b</li>\n</ul>\n"
        );
    }

    #[test]
    fn ordered_list() {
        assert_eq!(
            render("1. a\n2. b"),
            "<ol>\n<li>a</li>\n<li>b</li>\n</ol>\n"
        );
    }

    #[test]
    fn link() {
        assert_eq!(
            render("[text](https://example.com)"),
            "<p><a href=\"https://example.com\">text</a></p>\n"
        );
    }

    #[test]
    fn code_block_preserves_language_no_highlight() {
        let html = render("```rust\nlet x = 1;\n```");
        assert_eq!(
            html,
            "<pre><code class=\"language-rust\">let x = 1;\n</code></pre>\n"
        );
        // No syntax-highlight coloring: no nested <span> elements.
        assert!(!html.contains("<span"));
    }

    /// Highest-value test: guards the "GFM extensions enabled" decision.
    /// A pipe table must become a real <table>, not literal pipes.
    #[test]
    fn gfm_table() {
        let md = "| a | b |\n| - | - |\n| 1 | 2 |";
        let html = render(md);
        assert!(html.contains("<table>"), "expected a <table>, got: {html}");
        assert!(html.contains("<thead>"));
        assert!(html.contains("<tbody>"));
        assert!(!html.contains("| a | b |"), "table rendered as literal pipes");
    }

    #[test]
    fn gfm_strikethrough() {
        assert_eq!(render("~~gone~~"), "<p><del>gone</del></p>\n");
    }

    #[test]
    fn gfm_task_list() {
        let html = render("- [x] done");
        assert!(html.contains("type=\"checkbox\""), "got: {html}");
        assert!(html.contains("checked"), "got: {html}");
        assert!(html.contains("disabled"), "got: {html}");
    }

    #[test]
    fn gfm_autolink() {
        let html = render("see https://example.com here");
        assert!(
            html.contains("<a href=\"https://example.com\">https://example.com</a>"),
            "bare URL was not autolinked: {html}"
        );
    }

    #[test]
    fn plain_prose_is_paragraph() {
        assert_eq!(render("just words"), "<p>just words</p>\n");
    }

    #[test]
    fn empty_input_is_empty() {
        assert_eq!(render(""), "");
    }

    #[test]
    fn whitespace_input_is_noop() {
        assert_eq!(render("   \n  \n"), "");
    }

    /// Idempotency: because the plaintext fallback is the original markdown,
    /// re-running the tool feeds that markdown back through `render` and must
    /// produce byte-identical HTML — no double-escaping, no corruption.
    #[test]
    fn idempotent_via_fallback() {
        let md = "# Title\n\n**bold** and `code`\n\n| a | b |\n| - | - |\n| 1 | 2 |";
        let first = render(md);
        // The fallback carried on the clipboard is the *original markdown*,
        // so the second run renders the same input again.
        let second = render(md);
        assert_eq!(first, second);
    }
}
