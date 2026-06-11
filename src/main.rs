//! `mdclip` — read markdown from the clipboard, render it to HTML, and write
//! the HTML back with the original markdown retained as a plaintext fallback.
//!
//! Zero arguments, one shot. See `tdds/mdclip-v1.md` for the full design.

mod clipboard;
mod render;

use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    // Read first: a missing text flavor is a fatal error (clipboard untouched).
    let markdown = clipboard::read_text()?;

    // Empty clipboard string is a no-op, not an error.
    if markdown.is_empty() {
        return Ok(());
    }

    let html = render::render(&markdown);
    clipboard::write_html(&html, &markdown)?;

    Ok(())
}
