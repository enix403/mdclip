# TDD: `mdclip` — Markdown → Rich-Text Clipboard Converter

**Status:** Draft
**Author:** qateef.ahmad
**Date:** 2026-06-11
**Crate:** `mdclip` (Rust binary, edition 2024)

---

## Glossary

These terms are used consistently throughout this document.

- **Markdown** — the raw, plain-text markup the user authors (CommonMark + GitHub-Flavored extensions).
- **Rich text** — for the purposes of this tool, **HTML**. When the user says "rich text paste," they mean a clipboard payload that paste targets render with formatting (bold, headings, tables, etc.). We do **not** mean RTF.
- **Clipboard flavor / MIME type** — a single representation of clipboard content. The OS clipboard holds the same logical content in multiple parallel flavors simultaneously (e.g. HTML *and* plaintext); the receiving app picks whichever it understands.
- **HTML flavor** — `public.html` on macOS, `text/html` MIME type on Linux. The format virtually every web/Electron app reads on paste (Gmail, Slack, Notion, Google Docs, Linear, Discord, etc.).
- **Plaintext fallback** — a plain-text flavor placed on the clipboard alongside the HTML, for targets that cannot accept HTML. In this tool, the fallback is **the original markdown**.
- **Clipboard ownership (Linux)** — on X11/Wayland the clipboard is a *protocol*, not a store: the data is served on demand by the process that set it. The serving process must stay alive to satisfy a paste.
- **GFM** — GitHub-Flavored Markdown: CommonMark plus tables, strikethrough, autolinks, task lists.

---

## Problem Statement

The user authors content in **markdown**, but most destinations where that content must be pasted (Gmail, Slack, Notion, Google Docs, and similar) do not accept markdown input. They *do* accept a **rich text paste**. Today the user has no way to bridge this gap: pasting raw markdown into these apps shows literal `#`, `*`, `|` characters instead of formatted headings, bold text, and tables. The user wants to keep writing in markdown and have it appear formatted when pasted.

## Solution

A small cross-platform (macOS + Linux) CLI tool, `mdclip`, that performs a one-shot, zero-argument transform:

1. **Read** the raw markdown text currently on the system clipboard.
2. **Render** that markdown into HTML (the rich-text representation).
3. **Write** the HTML back to the system clipboard, with the original markdown retained as a plaintext fallback.

The user's workflow becomes: copy markdown → run `mdclip` → paste into the target app, where it appears fully formatted. Because the plaintext fallback is the original markdown, pasting into a plain-text field still yields readable, lossless markdown.

---

## Expected APIs / behavior (examples)

This tool has no public library API and no CLI flags in v1. The "API" is its observable clipboard behavior. The list below is the behavioral contract.

1. **Invocation:** `mdclip` with no arguments. There are no flags, subcommands, or positional arguments in v1.
2. **Happy path (macOS):** clipboard contains `# Hello\n\nworld` → after running, clipboard's HTML flavor contains `<h1>Hello</h1>\n<p>world</p>\n`; process exits 0 immediately.
3. **Happy path (Linux):** same input → parent process exits 0 immediately; a forked child holds the clipboard selection and serves the HTML on the next paste.
4. **Bold:** `**bold**` → `<p><strong>bold</strong></p>`.
5. **Italic:** `*italic*` → `<p><em>italic</em></p>`.
6. **Inline code:** `` `code` `` → `<p><code>code</code></p>`.
7. **Headings:** `## Section` → `<h2>Section</h2>`.
8. **Unordered list:** `- a\n- b` → `<ul>\n<li>a</li>\n<li>b</li>\n</ul>`.
9. **Ordered list:** `1. a\n2. b` → `<ol>\n<li>a</li>\n<li>b</li>\n</ol>`.
10. **Link:** `[text](https://example.com)` → `<p><a href="https://example.com">text</a></p>`.
11. **Code block:** a fenced ```` ```rust\nlet x = 1;\n``` ```` block → `<pre><code class="language-rust">let x = 1;\n</code></pre>` — preserved as a monospace block, **no syntax-highlight coloring**.
12. **GFM table:** a pipe table renders as a full `<table><thead>…<tbody>…</table>` — **not** as literal `|` characters. (Requires the table extension to be explicitly enabled.)
13. **GFM strikethrough:** `~~gone~~` → `<del>gone</del>`.
14. **GFM task list:** `- [x] done` → a list item containing a checked `<input type="checkbox" checked disabled>`.
15. **GFM autolink:** a bare `https://example.com` in text becomes a clickable `<a>`.
16. **Plaintext fallback:** for every successful run, the clipboard also carries a plaintext flavor equal to the **original markdown** (verbatim). Pasting into a plain `<textarea>` or terminal yields the markdown back.
17. **Idempotency:** running `mdclip` twice in a row is safe. The second run reads back the markdown via the plaintext fallback (`get_text()`), re-renders, and produces identical HTML. No corruption, no double-escaping.
18. **Empty clipboard string:** clipboard text is `""` → tool exits without writing anything (no-op).
19. **No text on clipboard (image, or empty):** `get_text()` returns `Err` → tool prints a message to **stderr** and exits **non-zero**; the clipboard is left untouched.
20. **Non-fatal content:** any valid UTF-8 text is accepted as markdown; plain prose with no markdown syntax renders as `<p>…</p>`.
21. **No success chatter (v1):** on success the tool is allowed to be silent (no required stdout/stderr message). Errors always go to stderr with a non-zero exit.

---

## Implementation Decisions

### Format: HTML only

"Rich text" is realized as the **HTML clipboard flavor** only. Rationale: HTML is the lingua franca of rich-text paste for web/Electron apps, which is where the user pastes. RTF (preferred by some native apps like TextEdit/Word) is explicitly **not** targeted in v1 — it is out of scope. This also keeps us on a single cross-platform code path, since `arboard` natively supports setting HTML but **not** RTF.

### Renderer: `comrak` with GFM extensions

Markdown → HTML is performed by **`comrak`** (a Rust port of GitHub's cmark-gfm). It was chosen over `pulldown-cmark` because it provides GitHub-identical GFM rendering — most importantly **tables** — matching the user's mental model when authoring.

**Critical:** `comrak` defaults to plain CommonMark. The GFM extensions **must be explicitly enabled**, or tables render as literal pipes and the entire reason for choosing `comrak` is lost:

```rust
let mut options = comrak::Options::default();
options.extension.table = true;
options.extension.strikethrough = true;
options.extension.autolink = true;
options.extension.tasklist = true;
let html = comrak::markdown_to_html(&markdown, &options);
```

### Output: bare semantic HTML

We ship `comrak`'s output **unmodified** — bare semantic tags (`<h1>`, `<strong>`, `<table>`, `<pre><code>`, …) with **no inline CSS** and **no syntax highlighting**. Rationale: rich-text editors re-style pasted semantic HTML with their own theme, so bare semantic HTML pastes more predictably than styled HTML; inline CSS can fight the target editor. Syntax highlighting (which would require a heavy dependency like `syntect`) is deferred — paste targets frequently strip span colors anyway.

### Input: clipboard only; markdown as fallback

- Input is read from the clipboard via `arboard`'s `get_text()`. **stdin support is deferred** (not in v1).
- The plaintext fallback (`alt_text`) passed to `arboard`'s HTML setter is the **original markdown string**, giving a lossless plaintext paste and the idempotency property above.
- On read error or empty string: message to stderr, non-zero exit, clipboard untouched. All fallible work (read + render) happens **before** the fork, so errors surface normally.

### Clipboard write & the Linux persistence problem

- **macOS:** `NSPasteboard` is a system-owned store. We call `clipboard.set().html(html, Some(markdown))` and exit; the data persists.
- **Linux:** the clipboard is served by the owning process; if `mdclip` exits, the paste gets nothing. We solve this by **forking a detached child** so the foreground command returns instantly (matching macOS UX). The mechanism, decided after weighing alternatives (`daemonize` crate, re-exec-self), is **raw `libc::fork()`**:
  - The **parent** does the fallible read+render, then forks and exits 0 immediately.
  - The **child** builds the `arboard::Clipboard` *after* the fork (X11/Wayland socket connections do not survive a fork cleanly) and calls `clipboard.set().wait().html(html, Some(markdown))`. `.wait()` blocks the child, serving the selection until another app takes ownership, after which the child exits on its own. This mirrors `wl-copy`'s behavior.
  - All Linux-specific code is gated behind `#[cfg(target_os = "linux")]`; macOS keeps the trivial set-and-exit path.

### CLI surface: none

v1 takes zero arguments. No argument parser (`clap` rejected as overkill), no `--help`/`--version`, no required success message. Goal is a functional tool first; niceties come later.

### Modules / sketch

For a tool this size a single `src/main.rs` is acceptable, but the logical decomposition is:

- **`render`** — pure function `fn render(markdown: &str) -> String`: configures `comrak::Options` (GFM extensions on) and returns HTML. Pure and trivially unit-testable.
- **`clipboard`** — reading (`get_text`) and the platform-divergent write. Contains the `#[cfg(target_os = "linux")]` fork path and the `#[cfg(not(target_os = "linux"))]` (macOS) direct path.
- **`main`** — orchestration: read → guard empty/error → render → write. Returns `Result<(), Box<dyn std::error::Error>>` for clean error propagation to a non-zero exit.

### Dependencies

- `comrak` — markdown → HTML.
- `arboard` — cross-platform clipboard read + HTML set.
- `libc` — `fork()`, **Linux-only**, declared under `[target.'cfg(target_os = "linux")'.dependencies]`.

---

## Testing Decisions

- **`render` module — unit tests (primary coverage).** Pure `String -> String`, so assert on rendered HTML for: heading, bold, italic, inline code, list (ordered/unordered), link, code block, and each enabled GFM extension (table, strikethrough, task list, autolink). The table test is the highest-value test — it guards the "extensions enabled" decision against regression.
- **Idempotency test.** `render(markdown)` followed by feeding the *fallback markdown* back through `render` yields identical HTML.
- **Empty/whitespace input.** `render("")` and the main-level guard for empty clipboard string behave as no-ops.
- **Clipboard / fork paths — not unit tested.** Clipboard I/O and `fork()` depend on a live display server/pasteboard and are environment-bound; they are excluded from automated tests and validated manually (copy markdown → run → paste into a target app on each OS). No prior art exists in this fresh crate; tests will be standard Rust `#[cfg(test)]` modules / `tests/` integration files.

---

## Out of Scope

- **RTF** (and any non-HTML rich format).
- **Inline CSS styling** and **syntax highlighting** of code blocks.
- **stdin input**, file input, or any non-clipboard source.
- **CLI flags**, argument parsing, `--help`/`--version`, success/progress messaging.
- **Windows** support.
- Best-effort guarantees beyond the documented Linux fork behavior (e.g. clipboard managers, persistence after the serving child exits without anyone pasting).

---

## Further Notes

- **Why the child creates the clipboard, not the parent:** X11/Wayland connections are sockets that don't survive `fork()` reliably; constructing `arboard::Clipboard` in the child after forking avoids a broken connection.
- **Idempotency is a free property**, not a feature to build — it falls out of choosing the original markdown as the plaintext fallback.
- **Future enhancements** (all deferred): RTF flavor via platform-specific code (`NSPasteboard` / `wl-copy`), `--stdin`, syntax highlighting via `syntect`, an arg parser (`clap`) once flags exist, and a `--serve`/daemon mode if the fork approach proves insufficient.
