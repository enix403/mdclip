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
///
/// Linux invariant (load-bearing): this builds the `Clipboard` in a local
/// scope and drops it before returning. On X11, `arboard::Clipboard::new()`
/// spawns a background serve-thread held in a process-global singleton; its
/// `Drop` joins that thread and clears the singleton. Dropping here is what
/// leaves the process single-threaded — and the global cache empty — by the
/// time `write_html` forks. Do not hoist this `Clipboard` up to share it with
/// the write path: see the SAFETY note in `write_html_linux`.
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

    // SAFETY: soundness rests on two invariants, not on `fork()` itself:
    //   1. Single-threaded at this point. `read_text()` built its arboard
    //      Clipboard in a temporary scope and dropped it before we got here;
    //      arboard's Drop joins the X11 serve-thread and clears its global
    //      singleton. So fork() strands no lock held by a vanished thread, and
    //      the child's Clipboard::new() builds a *fresh* connection rather than
    //      reusing the parent's via the global cache.
    //   2. The child never unwinds back into `main`: every branch below ends in
    //      process::exit, so the inherited copies of html/markdown are never
    //      dropped in the child (no double-free) and stay read-valid in its
    //      private copy-on-write copy of the address space.
    // Do NOT hold an arboard Clipboard open across this fork — it breaks both.
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
