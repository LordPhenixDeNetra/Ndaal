//! Rust crate rules: `#![forbid(unsafe_code)]` outside the exempt crates.

use super::{Rule, Violation};

/// Crates allowed to contain `unsafe` code (ARCHITECTURE.md section 3).
const UNSAFE_ALLOWED: [&str; 4] = ["ndaal-io", "ndaal-runtime", "ndaal-vector", "ndaal-wal"];

const FORBID_UNSAFE: &str = "#![forbid(unsafe_code)]";

/// Checks crate roots (`crates/<name>/src/lib.rs` or `main.rs`) given as (path, contents).
pub fn check_crate_roots(roots: &[(String, String)]) -> Vec<Violation> {
    roots
        .iter()
        .filter(|(path, contents)| !is_exempt(path) && !has_forbid_unsafe(contents))
        .map(|(path, _)| Violation {
            path: path.clone(),
            line: None,
            rule: Rule::MissingForbidUnsafe,
            detail: format!("add {FORBID_UNSAFE} (or justify an exemption in ARCHITECTURE.md)"),
        })
        .collect()
}

/// An actual `#![forbid(... unsafe_code ...)]` attribute line, not a comment mentioning it.
fn has_forbid_unsafe(contents: &str) -> bool {
    contents.lines().any(|line| {
        let line = line.trim_start();
        line.starts_with("#![forbid(") && line.contains("unsafe_code")
    })
}

/// `crates/<name>/...` where `<name>` may contain `unsafe`.
fn is_exempt(path: &str) -> bool {
    let name = path
        .strip_prefix("crates/")
        .and_then(|rest| rest.split('/').next());
    name.is_some_and(|name| UNSAFE_ALLOWED.contains(&name))
}

#[cfg(test)]
mod tests {
    use super::super::Rule;
    use super::check_crate_roots;

    fn rules(path: &str, content: &str) -> Vec<Rule> {
        check_crate_roots(&[(path.to_owned(), content.to_owned())])
            .into_iter()
            .map(|v| v.rule)
            .collect()
    }

    #[test]
    fn missing_forbid_unsafe_is_reported() {
        assert_eq!(
            rules("crates/ndaal-kv/src/lib.rs", "//! Kv.\n"),
            vec![Rule::MissingForbidUnsafe]
        );
    }

    #[test]
    fn commented_out_forbid_is_reported() {
        for root in [
            "// #![forbid(unsafe_code)] removed\n",
            "//! Uses #![forbid(unsafe_code)].\n",
        ] {
            assert_eq!(
                rules("crates/ndaal-kv/src/lib.rs", root),
                vec![Rule::MissingForbidUnsafe]
            );
        }
    }

    #[test]
    fn exempt_crates_may_omit_forbid() {
        for name in ["ndaal-io", "ndaal-runtime", "ndaal-vector", "ndaal-wal"] {
            let path = format!("crates/{name}/src/lib.rs");
            assert!(rules(&path, "//! Io.\n").is_empty(), "{name}");
        }
    }

    #[test]
    fn exempt_crate_with_forbid_is_fine() {
        assert!(rules("crates/ndaal-wal/src/lib.rs", "#![forbid(unsafe_code)]\n").is_empty());
    }

    #[test]
    fn cli_main_is_checked() {
        assert_eq!(
            rules("crates/ndaal-cli/src/main.rs", "fn main() {}\n"),
            vec![Rule::MissingForbidUnsafe]
        );
    }
}
