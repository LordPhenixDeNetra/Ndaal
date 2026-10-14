//! Development tasks for the Ndaal repository.
//!
//! Usage: `cargo run -p xtask -- <command>`:
//! - `hygiene`: repository hygiene (`scripts/check-hygiene.sh`);
//! - `links`: relative links and anchors of the Markdown files
//!   (`scripts/check-links.sh`);
//! - `book`: sources of the documentation site in `docs/book/src/`
//!   (`scripts/build-docs.sh`).

#![forbid(unsafe_code)]
// Command-line tool: printing is its output.
#![allow(clippy::print_stdout, clippy::print_stderr)]
// Development tool, not part of the engine: reads the repository directly.
#![allow(clippy::disallowed_methods, clippy::disallowed_types)]

mod docs;
mod hygiene;

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("hygiene") => run_hygiene(),
        Some("links") => run_links(),
        Some("book") => run_book(),
        _ => {
            eprintln!("usage: cargo run -p xtask -- hygiene|links|book");
            ExitCode::from(2)
        }
    }
}

/// Runs every hygiene rule on tracked and new (non-ignored) files.
fn run_hygiene() -> ExitCode {
    let (root, files) = match list_files() {
        Ok(listing) => listing,
        Err(message) => {
            eprintln!("hygiene: {message}");
            return ExitCode::from(2);
        }
    };
    let mut violations = Vec::new();
    let mut roots = Vec::new();
    for path in files {
        let bytes = match std::fs::read(root.join(&path)) {
            Ok(bytes) => bytes,
            // Deleted but not yet staged: nothing to check.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => {
                eprintln!("hygiene: cannot read {path}: {e}");
                return ExitCode::from(2);
            }
        };
        violations.extend(hygiene::check_file(&path, &bytes));
        if is_crate_root(&path) {
            roots.push((path, String::from_utf8_lossy(&bytes).into_owned()));
        }
    }
    violations.extend(hygiene::check_crate_roots(&roots));
    for violation in &violations {
        println!("{violation}");
    }
    if violations.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Reports every broken relative link of the Markdown files.
fn run_links() -> ExitCode {
    let (root, files) = match list_files() {
        Ok(listing) => listing,
        Err(message) => {
            eprintln!("links: {message}");
            return ExitCode::from(2);
        }
    };
    let mut contents = Vec::new();
    for path in files {
        // Only Markdown is parsed; other files only need to exist.
        let text = if path.ends_with(".md") {
            match std::fs::read_to_string(root.join(&path)) {
                Ok(text) => text,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => {
                    eprintln!("links: cannot read {path}: {e}");
                    return ExitCode::from(2);
                }
            }
        } else {
            String::new()
        };
        contents.push((path, text));
    }
    let broken = docs::links::check(&contents);
    for link in &broken {
        println!("{link}");
    }
    if broken.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Writes the mdBook sources: every page of the book and `SUMMARY.md`.
fn run_book() -> ExitCode {
    match write_book() {
        Ok(out) => {
            println!("book sources written to {}", out.display());
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("book: {message}");
            ExitCode::from(2)
        }
    }
}

fn write_book() -> Result<PathBuf, String> {
    let (root, _) = list_files()?;
    let out = root.join("docs").join("book").join("src");
    std::fs::create_dir_all(&out).map_err(|e| format!("cannot create {}: {e}", out.display()))?;
    for (document, _) in docs::book::BOOK_PAGES {
        let text = std::fs::read_to_string(root.join(document))
            .map_err(|e| format!("cannot read {document}: {e}"))?;
        let page = out.join(docs::book::book_name(document));
        std::fs::write(&page, docs::book::rewrite_links(document, &text))
            .map_err(|e| format!("cannot write {}: {e}", page.display()))?;
    }
    let summary = out.join("SUMMARY.md");
    std::fs::write(&summary, docs::book::summary())
        .map_err(|e| format!("cannot write {}: {e}", summary.display()))?;
    Ok(out)
}

/// The repository root and every file to check, relative to that root:
/// tracked files plus untracked, non-ignored ones (the maintainer commits
/// manually, so work in progress is not tracked yet). Independent of the
/// current directory, so running from a subdirectory still checks everything.
fn list_files() -> Result<(PathBuf, Vec<String>), String> {
    let top = git(&["rev-parse", "--show-toplevel"], None)?;
    let root = PathBuf::from(String::from_utf8_lossy(&top).trim());
    let listing = git(
        &[
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ],
        Some(&root),
    )?;
    let mut files: Vec<String> = listing
        .split(|&b| b == 0)
        .filter(|name| !name.is_empty())
        .map(|name| String::from_utf8_lossy(name).into_owned())
        .collect();
    files.sort();
    files.dedup();
    Ok((root, files))
}

/// Runs git and returns its standard output.
fn git(args: &[&str], dir: Option<&Path>) -> Result<Vec<u8>, String> {
    let mut command = Command::new("git");
    command.args(args);
    if let Some(dir) = dir {
        command.current_dir(dir);
    }
    let output = command
        .output()
        .map_err(|e| format!("cannot run git: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git {} failed: {}", args.join(" "), stderr.trim()));
    }
    Ok(output.stdout)
}

/// `crates/<name>/src/lib.rs` or `crates/<name>/src/main.rs`.
fn is_crate_root(path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    matches!(parts.as_slice(), ["crates", _, "src", "lib.rs" | "main.rs"])
}

#[cfg(test)]
mod tests {
    use super::list_files;

    // cargo test runs from xtask/, a subdirectory: paths must still be
    // relative to the repository root and cover the whole repository.
    #[test]
    fn listed_paths_are_relative_to_repository_root() {
        let (_, files) = list_files().expect("git ls-files");
        for expected in [
            "Cargo.toml",
            "xtask/src/main.rs",
            "crates/ndaal-kv/src/lib.rs",
        ] {
            assert!(files.iter().any(|f| f == expected), "{expected} not listed");
        }
    }
}
