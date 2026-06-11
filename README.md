# mdclip

Convert the markdown on your clipboard into rich text, in place.

Most apps where you paste — Gmail, Slack, Notion, Google Docs, Linear, Discord — don't understand markdown. Paste raw markdown and you get literal `#`, `*`, and `|` characters instead of headings, bold text, and tables. `mdclip` bridges that gap: it reads the markdown on your clipboard, renders it to HTML, and writes the HTML back. Paste afterwards and your content appears fully formatted.

## How it works

```
copy markdown  →  run mdclip  →  paste (formatted)
```

In one shot, `mdclip`:

1. **Reads** the raw markdown text on the system clipboard.
2. **Renders** it to HTML via [`comrak`](https://crates.io/crates/comrak) (GitHub-Flavored Markdown).
3. **Writes** the HTML back to the clipboard, keeping the original markdown as a plaintext fallback.

It takes no arguments by default; one optional flag, `--preserve-blank-lines`, is described under [Usage](#usage).

Because the plaintext fallback is the original markdown, pasting into a plain-text field (a terminal, a code editor, a `<textarea>`) still yields clean, readable markdown. Targets that understand HTML get the formatted version; everything else gets the markdown.

## Install

Requires a [Rust toolchain](https://rustup.rs/).

```sh
cargo install --path .
```

Or build a release binary directly:

```sh
cargo build --release
# binary at target/release/mdclip
```

## Usage

```sh
mdclip                         # render; blank-line runs collapse (CommonMark default)
mdclip --preserve-blank-lines  # keep every blank line between blocks (short: -b)
mdclip --help                  # print usage
```

A typical flow:

1. Select and copy some markdown.
2. Run `mdclip` (add `--preserve-blank-lines`, or `-b`, to keep your blank lines).
3. Paste into your target app.

On success the tool is silent and exits `0`. If the clipboard holds no text (it's empty or contains an image), `mdclip` prints a message to stderr, exits non-zero, and leaves the clipboard untouched. An empty clipboard string is a no-op. An unrecognized argument is an error.

### Blank lines

By default `mdclip` follows CommonMark: any run of blank lines between blocks collapses to a single paragraph break. Pass `--preserve-blank-lines` (`-b`) to keep them — each blank line in the source becomes a spacer paragraph (`<p>&nbsp;</p>`), so the vertical spacing you typed survives the paste. (The non-breaking space is deliberate: a truly empty `<p></p>` collapses to nothing in most paste targets.) Blank lines inside fenced code blocks are always kept verbatim; leading and trailing blank lines are always dropped.

## Supported markdown

CommonMark plus GitHub-Flavored extensions:

- Headings, bold, italic, inline code
- Ordered and unordered lists
- Links and bare-URL autolinks
- Fenced code blocks (preserved as monospace blocks; no syntax-highlight coloring)
- **Tables**
- Strikethrough (`~~text~~`)
- Task lists (`- [x] done`)

## Platform notes

- **macOS** — the clipboard is a system-owned store. `mdclip` sets the HTML and exits; the content persists.
- **Linux** — the clipboard is served on demand by the owning process (X11/Wayland). `mdclip` forks a detached background process that holds the selection and serves it on the next paste, then exits once another app takes ownership — mirroring how `wl-copy` behaves. The foreground command returns instantly.

Running `mdclip` twice in a row is safe: the second run reads the markdown back from the plaintext fallback, re-renders it, and produces identical output.

## Scope

`mdclip` deliberately does one thing. Out of scope for now: RTF and other non-HTML rich formats, inline CSS styling, syntax highlighting, stdin/file input, broader CLI configuration, and Windows support.

## Development

```sh
cargo test     # runs the renderer's unit tests
cargo clippy --all-targets
```

The renderer (`src/render.rs`) is a pure `&str -> String` function and is covered by unit tests for every supported markdown feature. Clipboard and fork behavior depend on a live display server / pasteboard and are validated manually on each platform.

## License

MIT
