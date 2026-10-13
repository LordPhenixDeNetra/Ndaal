//! Repository hygiene rules (CLAUDE.md sections 4.3, 4.6 and 4.7).
//!
//! The checks are pure functions over file paths and contents, so they are
//! unit-tested without touching the disk; `main.rs` does the I/O.

mod rust;
mod text;

use std::fmt;

pub use rust::check_crate_roots;
pub use text::check_file;

/// One rule broken by one file, optionally at one line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// Path relative to the repository root, with `/` separators.
    pub path: String,
    /// 1-based line number, when the rule applies to a single line.
    pub line: Option<usize>,
    /// The rule that is broken.
    pub rule: Rule,
    /// Human-readable detail (offending character, line count...).
    pub detail: String,
}

/// The hygiene rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rule {
    /// An emoji anywhere in the repository.
    Emoji,
    /// A non-ASCII character in a `.rs`, `.toml`, `.yml` or `.yaml` file.
    NonAscii,
    /// Spaces or tabs at the end of a line.
    TrailingWhitespace,
    /// A CRLF line ending outside Windows scripts.
    Crlf,
    /// A non-empty file that does not end with a newline.
    MissingFinalNewline,
    /// A Rust file over its line limit.
    FileTooLong,
    /// A crate root without `#![forbid(unsafe_code)]` outside the exempt crates.
    MissingForbidUnsafe,
    /// The ADR-017 side-effect lint silenced outside `ndaal-io` and `xtask`.
    ForbiddenLintBypass,
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Rule::Emoji => "emoji",
            Rule::NonAscii => "non-ascii",
            Rule::TrailingWhitespace => "trailing-whitespace",
            Rule::Crlf => "crlf",
            Rule::MissingFinalNewline => "missing-final-newline",
            Rule::FileTooLong => "file-too-long",
            Rule::MissingForbidUnsafe => "missing-forbid-unsafe",
            Rule::ForbiddenLintBypass => "forbidden-lint-bypass",
        };
        f.write_str(name)
    }
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(f, "{}:{}: {}: {}", self.path, line, self.rule, self.detail),
            None => write!(f, "{}: {}: {}", self.path, self.rule, self.detail),
        }
    }
}
