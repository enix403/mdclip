//! `mdclip` — read markdown from the clipboard, render it to HTML, and write
//! the HTML back with the original markdown retained as a plaintext fallback.
//!
//! One shot. By default blank lines between blocks are preserved; the optional
//! `--collapse-blank-lines` flag collapses them as plain CommonMark does.
//! See `tdds/mdclip-v1.md` for the core design.

mod clipboard;
mod render;

use std::env;
use std::error::Error;
use std::process::ExitCode;

const USAGE: &str = "\
Usage: mdclip [OPTIONS]

Render the markdown on the clipboard to HTML, in place.
Blank lines between blocks are kept by default.

Options:
  -c, --collapse-blank-lines  Collapse each run of blank lines into a single
                              break, as plain CommonMark does.
  -h, --help                  Print this help and exit.";

/// What the parsed command line asks us to do.
#[derive(Debug, PartialEq)]
enum Outcome {
    /// Do the conversion; `collapse_blank_lines` mirrors the flag.
    Run { collapse_blank_lines: bool },
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
            collapse_blank_lines,
        } => run(collapse_blank_lines),
    }
}

/// Parse the argument list (program name already stripped) into an [`Outcome`].
/// Unknown arguments are a hard error pointing at `--help`.
fn parse_args(args: impl Iterator<Item = String>) -> Result<Outcome, Box<dyn Error>> {
    let mut collapse_blank_lines = false;
    for arg in args {
        match arg.as_str() {
            "-c" | "--collapse-blank-lines" => collapse_blank_lines = true,
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
        collapse_blank_lines,
    })
}

/// Read the clipboard, render, and write it back. Blank lines are preserved by
/// default; `collapse_blank_lines` (the `--collapse-blank-lines` flag) skips
/// that pre-pass for plain CommonMark. The fallback is always the original
/// markdown.
fn run(collapse_blank_lines: bool) -> Result<(), Box<dyn Error>> {
    // Read first: a missing text flavor is a fatal error (clipboard untouched).
    let markdown = clipboard::read_text()?;

    // Empty clipboard string is a no-op, not an error.
    if markdown.is_empty() {
        return Ok(());
    }

    let html = if collapse_blank_lines {
        render::render(&markdown)
    } else {
        render::render(&render::preserve_blank_lines(&markdown))
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
    fn no_args_preserves_blank_lines_by_default() {
        assert_eq!(
            parse(&[]),
            Ok(Outcome::Run {
                collapse_blank_lines: false
            })
        );
    }

    #[test]
    fn flag_enables_collapse() {
        let on = Ok(Outcome::Run {
            collapse_blank_lines: true,
        });
        assert_eq!(parse(&["--collapse-blank-lines"]), on);
        assert_eq!(parse(&["-c"]), on);
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
