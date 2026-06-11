//! Clipboard I/O: reading the markdown, and the platform-divergent HTML write.
//!
//! Reading is identical everywhere. Writing differs:
//!
//! - **macOS:** `NSPasteboard` is a system-owned store; we set the HTML and
//!   exit, and the data persists on its own.
//! - **Linux:** the clipboard is served on demand by the owning process. If we
//!   exit, a subsequent paste gets nothing. We fork a detached child that
//!   builds its own `arboard::Clipboard` (X11/Wayland sockets do not survive a
//!   `fork()` cleanly) and blocks in `.wait()` to serve the selection until
//!   another app takes ownership. The parent returns instantly, matching the
//!   macOS UX.
//!
//! All fallible work (clipboard read) happens in the caller *before* the fork,
//! so errors surface normally with a non-zero exit.

use std::error::Error;

use arboard::Clipboard;

/// Read the current plaintext contents of the clipboard.
///
/// Returns `Err` when there is no text flavor available (e.g. the clipboard
/// holds an image, or is empty). The caller treats that as a fatal,
/// clipboard-untouched error.
pub fn read_text() -> Result<String, Box<dyn Error>> {
    let mut clipboard = Clipboard::new()?;
    Ok(clipboard.get_text()?)
}

/// Write `html` to the clipboard's HTML flavor, with `alt_text` (the original
/// markdown) as the plaintext fallback.
///
/// On macOS this sets and returns. On Linux this forks a detached child that
/// serves the selection; the parent returns immediately.
pub fn write_html(html: &str, alt_text: &str) -> Result<(), Box<dyn Error>> {
    #[cfg(target_os = "linux")]
    {
        write_html_linux(html, alt_text)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let mut clipboard = Clipboard::new()?;
        clipboard.set().html(html, Some(alt_text))?;
        Ok(())
    }
}

/// Linux write path: fork a detached child to hold and serve the selection.
///
/// The parent forks and returns `Ok(())` immediately. The child constructs the
/// clipboard *after* the fork, then `.wait()`s — blocking until another app
/// claims the selection — and exits the process directly so its return value
/// never propagates up through `main`.
#[cfg(target_os = "linux")]
fn write_html_linux(html: &str, alt_text: &str) -> Result<(), Box<dyn Error>> {
    use arboard::SetExtLinux;

    // SAFETY: `fork()` is inherently unsafe. We do no allocation between the
    // fork and the point where each branch resumes normal Rust: the parent
    // returns, and the child immediately builds fresh state and runs to
    // process exit, never returning to a parent with a now-invalid heap view.
    let pid = unsafe { libc::fork() };

    if pid < 0 {
        return Err("fork() failed".into());
    }

    if pid > 0 {
        // Parent: hand off to the child and return instantly.
        return Ok(());
    }

    // Child. Build the clipboard connection *after* the fork — X11/Wayland
    // sockets do not survive a fork cleanly. `.wait()` blocks, serving the
    // selection until another app takes ownership, then returns.
    match Clipboard::new()
        .and_then(|mut clipboard| clipboard.set().wait().html(html, Some(alt_text)))
    {
        Ok(()) => std::process::exit(0),
        // Nothing useful to do with an error here: the parent has already
        // exited 0 and there is no terminal attached to this detached child.
        Err(_) => std::process::exit(1),
    }
}
