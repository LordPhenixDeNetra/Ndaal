//! Development tasks for the Ndaal repository.
//!
//! Usage: `cargo run -p xtask -- <command>`:
//! - `hygiene`: repository hygiene (`scripts/check-hygiene.sh`);
//! - `links`: relative links and anchors of the Markdown files
//!   (`scripts/check-links.sh`);
//! - `book`: sources of the documentation site in `docs/book/src/`
//!   (`scripts/build-docs.sh`);
//! - `compare <dir>`: verdict and report of a comparative benchmark run
//!   (`bench/compare/`).

#![forbid(unsafe_code)]
// Command-line tool: printing is its output.
#![allow(clippy::print_stdout, clippy::print_stderr)]
// Development tool, not part of the engine: reads the repository directly.
#![allow(clippy::disallowed_methods, clippy::disallowed_types)]

mod compare;
mod docs;
mod hygiene;

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("hygiene") => run_hygiene(),
        Some("links") => run_links(),
        Some("book") => run_book(),
        Some("compare") => match std::env::args().nth(2) {
            Some(dir) => run_compare(Path::new(&dir)),
            None => {
                eprintln!("usage: cargo run -p xtask -- compare <results dir>");
                ExitCode::from(2)
            }
        },
        _ => {
            eprintln!("usage: cargo run -p xtask -- hygiene|links|book|compare");
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

/// Compares the runs of `dir` (`metadata.json`, `reference-*.json`,
/// `candidate-*.json`), writes `report.md` and prints it. Exit code 0 within
/// the tolerance, 1 beyond it, 2 on invalid input.
fn run_compare(dir: &Path) -> ExitCode {
    match compare_dir(dir) {
        Ok((report, within)) => {
            println!("{report}");
            if within {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(message) => {
            eprintln!("compare: {message}");
            ExitCode::from(2)
        }
    }
}

fn compare_dir(dir: &Path) -> Result<(String, bool), String> {
    let read = |path: &Path| {
        std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))
    };
    let metadata: serde_json::Value = serde_json::from_str(&read(&dir.join("metadata.json"))?)
        .map_err(|e| format!("invalid metadata.json: {e}"))?;
    let mut sides = [Vec::new(), Vec::new()];
    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("cannot list {}: {e}", dir.display()))?;
    let mut paths: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    paths.sort();
    for path in paths {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        let side = match name {
            n if n.starts_with("reference-") && n.ends_with(".json") => 0,
            n if n.starts_with("candidate-") && n.ends_with(".json") => 1,
            _ => continue,
        };
        let sample = compare::memtier::parse(&read(&path)?).map_err(|e| format!("{name}: {e}"))?;
        sides[side].push(sample);
    }
    let repeats = metadata.get("repeats").and_then(serde_json::Value::as_u64);
    for (name, side) in [("reference", &sides[0]), ("candidate", &sides[1])] {
        if repeats != Some(side.len() as u64) {
            return Err(format!(
                "{} {name} runs, metadata.json repeats is {repeats:?}: runs of another invocation?",
                side.len()
            ));
        }
    }
    let verdict = compare::verdict::compare(&sides[0], &sides[1])?;
    let bench = metadata
        .get("bench")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unnamed");
    let report = compare::verdict::report(bench, &metadata, &verdict);
    let out = dir.join("report.md");
    std::fs::write(&out, &report).map_err(|e| format!("cannot write {}: {e}", out.display()))?;
    Ok((report, verdict.within_tolerance))
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
    use super::{compare_dir, list_files};

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

    fn results(dir: &std::path::Path, repeats: usize, per_side: usize) {
        let fixture = include_str!("../tests/fixtures/memtier-2.5.2.json");
        let metadata = format!("{{\"bench\": \"kv\", \"repeats\": {repeats}}}");
        std::fs::write(dir.join("metadata.json"), metadata).expect("metadata");
        for side in ["reference", "candidate"] {
            for i in 1..=per_side {
                std::fs::write(dir.join(format!("{side}-{i}.json")), fixture).expect("run");
            }
        }
    }

    #[test]
    fn compare_dir_writes_the_report() {
        let dir = tempfile::tempdir().expect("tempdir");
        results(dir.path(), 2, 2);
        std::fs::write(dir.path().join("notes.txt"), "ignored").expect("notes");
        let (report, within) = compare_dir(dir.path()).expect("valid");
        assert!(within, "identical runs");
        assert!(report.contains("Medians of 2 runs per side"), "{report}");
        assert!(dir.path().join("report.md").exists());
    }

    /// Runs left by an earlier invocation must not mix in silently.
    #[test]
    fn compare_dir_checks_the_number_of_runs() {
        let dir = tempfile::tempdir().expect("tempdir");
        results(dir.path(), 2, 3);
        let error = compare_dir(dir.path()).expect_err("3 runs, 2 announced");
        assert!(error.contains("repeats"), "{error}");
    }

    #[test]
    fn compare_dir_names_the_broken_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        results(dir.path(), 1, 1);
        std::fs::write(dir.path().join("candidate-1.json"), "{}").expect("broken");
        let error = compare_dir(dir.path()).expect_err("broken run");
        assert!(error.contains("candidate-1.json"), "{error}");
    }
}
