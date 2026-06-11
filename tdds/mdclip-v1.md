# TDD: `mdclip` — Markdown → Rich-Text Clipboard Converter

**Status:** Implemented
**Author:** qateef.ahmad
**Date:** 2026-06-11
**Crate:** `mdclip` (Rust binary, edition 2024)

> **Revision (2026-06-11):** Blank-line preservation is now the **default**
> rendering behavior, with `--collapse-blank-lines` (`-c`) to opt out, plus
> `-h`/`--help`. This added a minimal hand-rolled argument parser (still no
> `clap`). The sections below describe the shipped behavior; the original v1
> draft specified zero arguments and plain CommonMark output.

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

A small cross-platform (macOS + Linux) CLI tool, `mdclip`, that performs a one-shot transform (no arguments required):

1. **Read** the raw markdown text currently on the system clipboard.
2. **Render** that markdown into HTML (the rich-text representation), keeping the blank lines between blocks the user typed by default.
3. **Write** the HTML back to the system clipboard, with the original markdown retained as a plaintext fallback.

The user's workflow becomes: copy markdown → run `mdclip` → paste into the target app, where it appears fully formatted. Because the plaintext fallback is the original markdown, pasting into a plain-text field still yields readable, lossless markdown.

---

## Expected APIs / behavior (examples)

This tool has no public library API; its "API" is its observable clipboard behavior plus a tiny CLI surface (one flag and `--help`). The list below is the behavioral contract.

1. **Invocation:** `mdclip` with no arguments does the conversion. One optional flag, `-c`/`--collapse-blank-lines`, opts out of blank-line preservation; `-h`/`--help` prints usage and exits 0; any other argument is an error (message to stderr, non-zero exit). No subcommands or positional arguments.
2. **Happy path (macOS):** clipboard contains `# Hello\n\nworld` → after running with the default (blank lines preserved), the HTML flavor contains `<h1>Hello</h1>\n<p>&nbsp;</p>\n<p>world</p>\n` (the blank line becomes a `&nbsp;` spacer — a non-breaking space); with `--collapse-blank-lines` it is `<h1>Hello</h1>\n<p>world</p>\n`. Process exits 0 immediately.
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
22. **Blank-line preservation (default):** `A\n\n\n\nB` (three blank lines between the paragraphs) → `<p>A</p>\n<p>&nbsp;</p>\n<p>&nbsp;</p>\n<p>&nbsp;</p>\n<p>B</p>\n` — one `&nbsp;` spacer paragraph per source blank line (a non-breaking space, so the line stays visible in paste targets that collapse empty `<p>`). Blank lines inside fenced code blocks are left verbatim; leading/trailing blank lines are dropped.
23. **Collapse opt-out:** with `-c`/`--collapse-blank-lines`, the same input renders as plain CommonMark — `<p>A</p>\n<p>B</p>\n`.
24. **Help:** `-h`/`--help` prints a usage summary to **stdout** and exits **0**; the clipboard is left untouched.
25. **Unrecognized argument:** e.g. `mdclip --nope` prints ``mdclip: unrecognized argument '--nope'; run `mdclip --help` for usage`` to **stderr** and exits **non-zero**; the clipboard is left untouched.

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

(One transform happens before `render`, not after: see blank-line preservation below.)

### Blank-line preservation (default), with an opt-out

CommonMark collapses any run of blank lines between blocks into a single paragraph break, discarding the rest. `mdclip` keeps them **by default**: a pre-pass (`render::preserve_blank_lines`, run *before* `render` because the parser drops the blank-line count) turns each blank line in a run into a `&nbsp;` spacer paragraph. The non-breaking space is deliberate — a truly empty `<p></p>` collapses to nothing in most paste targets, so the spacer would be invisible; one spacer is emitted *per* blank line, because targets stack `<p>`s tightly and the inter-paragraph gap isn't itself a visible line.

The pre-pass deliberately leaves two things alone: blank lines inside **fenced code blocks** (which `comrak` already preserves verbatim, and where injecting `&nbsp;` would corrupt the code) and **leading/trailing** blank lines (dropped, as markdown does). It is kept *separate* from `render` — `render` stays a pure CommonMark+GFM transform and `main` composes the two — and `--collapse-blank-lines` (`-c`) simply skips it for standard CommonMark output.

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

### CLI surface: minimal, hand-rolled

The default invocation takes no arguments. A small hand-rolled parser (no `clap` — still overkill for this surface) recognizes `-c`/`--collapse-blank-lines` and `-h`/`--help`; any other argument is a hard error pointing at `--help`. There is no `--version` and no required success message. `main` returns `ExitCode`; a `dispatch` helper does the fallible work and `main` maps its `Result`, printing errors as `mdclip: <message>` (the error's `Display`) — cleaner than the `Error: "…"` `Debug` output a `Result`-returning `main` emits.

### Modules / sketch

For a tool this size a single `src/main.rs` is acceptable, but the logical decomposition is:

- **`render`** — `pub fn render(markdown: &str) -> String`: configures `comrak::Options` (GFM extensions on) and returns HTML. Pure and trivially unit-testable. Alongside it, `pub fn preserve_blank_lines(markdown: &str) -> String` is the blank-line pre-pass (default-on, applied by `main`), kept separate so `render` stays pure.
- **`clipboard`** — reading (`get_text`) and the platform-divergent write. Contains the `#[cfg(target_os = "linux")]` fork path and the `#[cfg(not(target_os = "linux"))]` (macOS) direct path.
- **`main`** — argument parsing (`parse_args` → an `Outcome` enum) and orchestration: parse → read → guard empty/error → (optionally preserve) → render → write. Returns `ExitCode`; a `dispatch` helper performs the fallible work and `main` maps its `Result` to an exit code, printing errors as `mdclip: <message>`.

### Dependencies

- `comrak` — markdown → HTML.
- `arboard` — cross-platform clipboard read + HTML set.
- `libc` — `fork()`, **Linux-only**, declared under `[target.'cfg(target_os = "linux")'.dependencies]`.

---

## Testing Decisions

- **`render` module — unit tests (primary coverage).** Pure `String -> String`, so assert on rendered HTML for: heading, bold, italic, inline code, list (ordered/unordered), link, code block, and each enabled GFM extension (table, strikethrough, task list, autolink). The table test is the highest-value test — it guards the "extensions enabled" decision against regression.
- **Idempotency test.** `render(markdown)` followed by feeding the *fallback markdown* back through `render` yields identical HTML.
- **Empty/whitespace input.** `render("")` and the main-level guard for empty clipboard string behave as no-ops.
- **Blank-line preservation tests.** The pre-pass + renderer pipeline: one `&nbsp;` spacer per blank line for single and multi-blank runs, fenced-code blanks left verbatim, leading/trailing blanks dropped — plus the bare renderer collapsing runs (the `--collapse-blank-lines` path).
- **Argument parsing tests.** `parse_args` maps no args → preserve (collapse off), `-c`/`--collapse-blank-lines` → collapse on, `-h`/`--help` → help, and an unknown argument → error.
- **Clipboard / fork paths — not unit tested.** Clipboard I/O and `fork()` depend on a live display server/pasteboard and are environment-bound; they are excluded from automated tests and validated manually (copy markdown → run → paste into a target app on each OS). No prior art exists in this fresh crate; tests will be standard Rust `#[cfg(test)]` modules / `tests/` integration files.

---

## Out of Scope

- **RTF** (and any non-HTML rich format).
- **Inline CSS styling** and **syntax highlighting** of code blocks.
- **stdin input**, file input, or any non-clipboard source.
- **`--version`**, broader configuration or subcommands, and success/progress messaging. (A single `--collapse-blank-lines` flag, `--help`, and a minimal argument parser are now *in* scope — see "CLI surface" above.)
- **Windows** support.
- Best-effort guarantees beyond the documented Linux fork behavior (e.g. clipboard managers, persistence after the serving child exits without anyone pasting).

---

## Further Notes

- **Why the child creates the clipboard, not the parent:** X11/Wayland connections are sockets that don't survive `fork()` reliably; constructing `arboard::Clipboard` in the child after forking avoids a broken connection.
- **Idempotency is a free property**, not a feature to build — it falls out of choosing the original markdown as the plaintext fallback.
- **Future enhancements** (all deferred): RTF flavor via platform-specific code (`NSPasteboard` / `wl-copy`), `--stdin`, syntax highlighting via `syntect`, adopting an arg parser (`clap`) if the CLI outgrows the current hand-rolled handful of flags, and a `--serve`/daemon mode if the fork approach proves insufficient.
