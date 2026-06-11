//! `mdclip` — read markdown from the clipboard, render it to HTML, and write
//! the HTML back with the original markdown retained as a plaintext fallback.
//!
//! One shot. Takes no arguments by default; the optional
//! `--preserve-blank-lines` flag keeps blank lines instead of collapsing them.
//! See `tdds/mdclip-v1.md` for the core design.

mod clipboard;
mod render;

use std::env;
use std::error::Error;
use std::process::ExitCode;

const USAGE: &str = "\
Usage: mdclip [OPTIONS]

Render the markdown on the clipboard to HTML, in place.

Options:
  -b, --preserve-blank-lines  Keep every blank line between blocks (each becomes
                              a spacer) instead of collapsing each run to one.
  -h, --help                  Print this help and exit.";

/// What the parsed command line asks us to do.
#[derive(Debug, PartialEq)]
enum Outcome {
    /// Do the conversion; `preserve_blank_lines` mirrors the flag.
    Run { preserve_blank_lines: bool },
    /// `-h`/`--help` was given — print usage and exit successfully.
    HelpRequested,
}

fn main() -> ExitCode {
    // Errors print as `mdclip: <message>` (Display) — cleaner than the
    // `Error: "..."` Debug formatting a `Result`-returning `main` would emit.
    match dispatch() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("mdclip: {e}");
            ExitCode::FAILURE
        }
    }
}

fn dispatch() -> Result<(), Box<dyn Error>> {
    match parse_args(env::args().skip(1))? {
        Outcome::HelpRequested => {
            println!("{USAGE}");
            Ok(())
        }
        Outcome::Run {
            preserve_blank_lines,
        } => run(preserve_blank_lines),
    }
}

/// Parse the argument list (program name already stripped) into an [`Outcome`].
/// Unknown arguments are a hard error pointing at `--help`.
fn parse_args(args: impl Iterator<Item = String>) -> Result<Outcome, Box<dyn Error>> {
    let mut preserve_blank_lines = false;
    for arg in args {
        match arg.as_str() {
            "-b" | "--preserve-blank-lines" => preserve_blank_lines = true,
            "-h" | "--help" => return Ok(Outcome::HelpRequested),
            other => {
                return Err(format!(
                    "unrecognized argument '{other}'; run `mdclip --help` for usage"
                )
                .into());
            }
        }
    }
    Ok(Outcome::Run {
        preserve_blank_lines,
    })
}

/// Read the clipboard, render, and write it back. `preserve_blank_lines` opts
/// into the blank-line pre-pass; the fallback is always the original markdown.
fn run(preserve_blank_lines: bool) -> Result<(), Box<dyn Error>> {
    // Read first: a missing text flavor is a fatal error (clipboard untouched).
    let markdown = clipboard::read_text()?;

    // Empty clipboard string is a no-op, not an error.
    if markdown.is_empty() {
        return Ok(());
    }

    let html = if preserve_blank_lines {
        render::render(&render::preserve_blank_lines(&markdown))
    } else {
        render::render(&markdown)
    };
    clipboard::write_html(&html, &markdown)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Outcome, parse_args};

    fn parse(args: &[&str]) -> Result<Outcome, String> {
        parse_args(args.iter().map(|s| s.to_string())).map_err(|e| e.to_string())
    }

    #[test]
    fn no_args_runs_with_preserve_off() {
        assert_eq!(
            parse(&[]),
            Ok(Outcome::Run {
                preserve_blank_lines: false
            })
        );
    }

    #[test]
    fn flag_enables_preserve() {
        let on = Ok(Outcome::Run {
            preserve_blank_lines: true,
        });
        assert_eq!(parse(&["--preserve-blank-lines"]), on);
        assert_eq!(parse(&["-b"]), on);
    }

    #[test]
    fn help_flag_requests_help() {
        assert_eq!(parse(&["--help"]), Ok(Outcome::HelpRequested));
        assert_eq!(parse(&["-h"]), Ok(Outcome::HelpRequested));
    }

    #[test]
    fn unrecognized_argument_is_an_error() {
        assert!(parse(&["--nope"]).is_err());
    }
}
