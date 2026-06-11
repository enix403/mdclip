//! Markdown -> HTML rendering.
//!
//! A single pure function, [`render`], that turns markdown into the bare
//! semantic HTML we place on the clipboard. GFM extensions (tables,
//! strikethrough, autolinks, task lists) are explicitly enabled — without
//! them `comrak` falls back to plain CommonMark and tables render as literal
//! pipes, defeating the reason `comrak` was chosen.
//!
//! [`preserve_blank_lines`] is an *optional* pre-pass the binary applies to the
//! markdown before [`render`] when the `--preserve-blank-lines` flag is set;
//! see its docs. It is deliberately not folded into [`render`] so that the
//! renderer stays a pure CommonMark+GFM transform.

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

/// Keep the blank lines the user typed. Opt-in pre-pass, applied by `main` only
/// under the `--preserve-blank-lines` flag; the default path skips it.
///
/// CommonMark treats any run of consecutive blank lines between blocks as a
/// single block separator and discards the rest, so `A\n\n\n\nB` renders the
/// same as `A\n\nB`. When the user opts in, the extra blank lines are deliberate
/// vertical spacing to keep. The blank-line *count* only survives in the raw
/// source (the parser drops it), so this runs *before* [`render`]: every blank
/// line in a run becomes a `&nbsp;` spacer paragraph, which `comrak` renders as
/// `<p>\u{00a0}</p>`. The non-breaking space matters — a truly empty `<p></p>`
/// collapses to nothing in most paste targets, so it would be invisible; one
/// with content holds its line. One spacer *per* blank line (not per blank line
/// beyond the first): paste targets stack `<p>`s tightly, so the gap between
/// two paragraphs isn't itself a visible line — only a spacer is. Anything less
/// renders one blank line short of the source.
///
/// Two cases are deliberately left alone:
/// - **Fenced code blocks**, where `comrak` already preserves blank lines
///   verbatim and injecting `&nbsp;` would corrupt the code.
/// - **Leading/trailing** blank lines, which markdown discards anyway; only
///   runs *between* content are expanded.
pub fn preserve_blank_lines(markdown: &str) -> String {
    let lines: Vec<&str> = markdown.split('\n').collect();
    let is_blank = |line: &str| line.trim().is_empty();

    // Everything before the first / after the last non-blank line is
    // leading/trailing whitespace markdown drops anyway — bail if there is no
    // content at all so empty / whitespace-only input stays a no-op.
    let (Some(first), Some(last)) = (
        lines.iter().position(|l| !is_blank(l)),
        lines.iter().rposition(|l| !is_blank(l)),
    ) else {
        return markdown.to_string();
    };

    let mut out: Vec<&str> = Vec::with_capacity(lines.len());
    let mut in_code_fence = false;
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if is_code_fence(line) {
            in_code_fence = !in_code_fence;
            out.push(line);
            i += 1;
        } else if !in_code_fence && is_blank(line) && i > first && i < last {
            // A run of blank lines strictly inside the content region: emit one
            // spacer paragraph per blank line so the gap is preserved exactly.
            // The interleaved "" lines keep each &nbsp; a separate paragraph.
            let start = i;
            while i < last && is_blank(lines[i]) {
                i += 1;
            }
            out.push("");
            for _ in 0..(i - start) {
                out.push("&nbsp;");
                out.push("");
            }
        } else {
            out.push(line);
            i += 1;
        }
    }
    out.join("\n")
}

/// Whether `line` opens or closes a fenced code block (` ``` ` or `~~~`,
/// indented up to three spaces per CommonMark). Used only to decide where *not*
/// to inject spacers; fence pairing by length is left to `comrak`.
fn is_code_fence(line: &str) -> bool {
    let trimmed = line.trim_start_matches(' ');
    let indent = line.len() - trimmed.len();
    indent <= 3 && (trimmed.starts_with("```") || trimmed.starts_with("~~~"))
}

#[cfg(test)]
mod tests {
    use super::{preserve_blank_lines, render};

    /// The opt-in pipeline the binary runs under `--preserve-blank-lines`:
    /// the pre-pass, then the renderer. Default-path tests call `render` alone.
    fn preserved(markdown: &str) -> String {
        render(&preserve_blank_lines(markdown))
    }

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

    /// Default path (flag off): blank-line runs collapse, as plain CommonMark.
    /// This is the behavior `preserve_blank_lines` opts out of.
    #[test]
    fn blank_runs_collapse_by_default() {
        assert_eq!(render("A\n\n\n\nB"), "<p>A</p>\n<p>B</p>\n");
    }

    /// With the pre-pass, one blank line in the source is one blank line in the
    /// output: a single `&nbsp;` spacer paragraph (`\u{a0}`). Paste targets
    /// stack `<p>`s tightly, so without the spacer this gap would vanish.
    #[test]
    fn preserve_single_blank_line_becomes_one_spacer() {
        assert_eq!(preserved("A\n\nB"), "<p>A</p>\n<p>\u{a0}</p>\n<p>B</p>\n");
    }

    /// The bug this guards: blank-line runs used to collapse. With the pre-pass
    /// the spacer count matches the source exactly — three source blank lines
    /// yield three `&nbsp;` spacers, not two (which would render one line short).
    #[test]
    fn preserve_blank_run_one_spacer_per_line() {
        assert_eq!(
            preserved("A\n\n\n\nB"),
            "<p>A</p>\n<p>\u{a0}</p>\n<p>\u{a0}</p>\n<p>\u{a0}</p>\n<p>B</p>\n"
        );
    }

    /// Even with the pre-pass, blank lines inside a fenced code block are the
    /// code's own; `comrak` keeps them verbatim and we must not inject `&nbsp;`.
    #[test]
    fn preserve_leaves_code_block_blanks_untouched() {
        assert_eq!(
            preserved("```\nx\n\n\ny\n```"),
            "<pre><code>x\n\n\ny\n</code></pre>\n"
        );
    }

    /// Only runs *between* content are expanded; leading/trailing blank lines
    /// are dropped exactly as CommonMark does, even with the pre-pass.
    #[test]
    fn preserve_drops_leading_and_trailing_blanks() {
        assert_eq!(preserved("\n\n\nA\n\n\n"), "<p>A</p>\n");
        // Whitespace-only input stays a no-op through the pre-pass too.
        assert_eq!(preserved("   \n  \n"), "");
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
