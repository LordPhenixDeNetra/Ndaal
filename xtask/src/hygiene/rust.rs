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

/// Lints that enforce ADR-017 (no direct clock, disk, network or randomness).
/// Lints whose silencing disables the ADR-017 side-effect check: the two
/// lints themselves, and the groups they belong to.
const SIDE_EFFECT_LINTS: [&str; 4] = [
    "clippy::disallowed_methods",
    "clippy::disallowed_types",
    "clippy::all",
    "clippy::style",
];

/// Places allowed to silence them: the I/O layer itself, and development tools.
const LINT_BYPASS_ALLOWED: [&str; 2] = ["crates/ndaal-io/", "xtask/"];

/// Single files allowed to silence them: the deterministic hash map aliases
/// (P-31) name `std::collections::HashMap` with a fixed hasher.
const LINT_BYPASS_FILES: [&str; 1] = ["crates/ndaal-common/src/collections.rs"];

/// `lint` appears in `compact` as a whole path (`clippy::all` is not
/// `clippy::allow_attributes`).
fn mentions(compact: &str, lint: &str) -> bool {
    compact.match_indices(lint).any(|(at, _)| {
        compact[at + lint.len()..]
            .chars()
            .next()
            .is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '_'))
    })
}

fn bypass(path: &str, detail: String) -> Option<Violation> {
    Some(Violation {
        path: path.to_owned(),
        line: None,
        rule: Rule::ForbiddenLintBypass,
        detail,
    })
}

/// Reports a `.rs` file that silences the side-effect lints outside the
/// allowed places. Whitespace is ignored (`clippy :: all` is still `clippy::all`).
pub(super) fn lint_bypass(path: &str, contents: &str) -> Option<Violation> {
    if LINT_BYPASS_ALLOWED
        .iter()
        .any(|prefix| path.starts_with(prefix))
        || LINT_BYPASS_FILES.contains(&path)
    {
        return None;
    }
    let compact: String = contents.chars().filter(|c| !c.is_whitespace()).collect();
    let lint = SIDE_EFFECT_LINTS
        .iter()
        .find(|lint| mentions(&compact, lint))?;
    bypass(
        path,
        format!("{lint} may only be allowed in ndaal-io (ADR-017)"),
    )
}

/// Reports configuration that would switch the side-effect lints off without
/// touching any `.rs` file: a second `clippy.toml` (it replaces the root one
/// for its crate), a member manifest with its own lint table, or a Cargo
/// config passing clippy flags.
pub(super) fn config_bypass(path: &str, contents: &str) -> Option<Violation> {
    let compact: String = contents.chars().filter(|c| !c.is_whitespace()).collect();
    let file = path.rsplit('/').next().unwrap_or(path);
    if (file == "clippy.toml" || file == ".clippy.toml") && path != "clippy.toml" {
        return bypass(path, "only the root clippy.toml may exist (ADR-017)".into());
    }
    let member = path.starts_with("crates/") || path.starts_with("xtask/");
    if member && file == "Cargo.toml" && !compact.contains("[lints]workspace=true") {
        return bypass(
            path,
            "member crates must use `[lints] workspace = true`".into(),
        );
    }
    if path.starts_with(".cargo/config") && compact.contains("clippy") {
        return bypass(path, "Cargo config must not change clippy lints".into());
    }
    None
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

#[cfg(test)]
mod lint_bypass_tests {
    use super::super::Rule;
    use super::lint_bypass;

    const ALLOW: &str = "#![allow(clippy::disallowed_methods, clippy::disallowed_types)]\n";

    #[test]
    fn bypass_outside_ndaal_io_is_reported() {
        for path in [
            "crates/ndaal-kv/src/lib.rs",
            "crates/ndaal-sim/src/scenario.rs",
        ] {
            let found = lint_bypass(path, ALLOW).map(|v| v.rule);
            assert_eq!(found, Some(Rule::ForbiddenLintBypass), "{path}");
        }
        let types_only = "#[allow(clippy::disallowed_types)]\nfn f() {}\n";
        assert!(lint_bypass("crates/ndaal-kv/src/x.rs", types_only).is_some());
    }

    #[test]
    fn bypass_is_allowed_in_ndaal_io_and_xtask() {
        for path in [
            "crates/ndaal-io/src/lib.rs",
            "crates/ndaal-io/tests/real_net.rs",
            "xtask/src/main.rs",
            "crates/ndaal-common/src/collections.rs",
        ] {
            assert!(lint_bypass(path, ALLOW).is_none(), "{path}");
        }
        for path in [
            "crates/ndaal-common/src/collections.rs.bak",
            "crates/ndaal-common/src/ids.rs",
        ] {
            assert!(lint_bypass(path, ALLOW).is_some(), "{path}");
        }
    }

    #[test]
    fn files_without_bypass_pass() {
        assert!(lint_bypass("crates/ndaal-kv/src/lib.rs", "fn f() {}\n").is_none());
    }
}

#[cfg(test)]
mod lint_bypass_hardening_tests {
    use super::{config_bypass, lint_bypass};

    #[test]
    fn spaced_paths_and_group_allows_are_reported() {
        for source in [
            "#[allow(clippy :: disallowed_methods)]
fn f() {}
",
            "#![allow(clippy::all)]
",
            "#[expect(clippy::style)]
fn f() {}
",
            "#![cfg_attr(test, allow(clippy::disallowed_types))]
",
        ] {
            assert!(
                lint_bypass("crates/ndaal-kv/src/lib.rs", source).is_some(),
                "{source:?}"
            );
        }
    }

    #[test]
    fn only_the_root_clippy_config_is_allowed() {
        assert!(
            config_bypass(
                "clippy.toml",
                "x = 1
"
            )
            .is_none()
        );
        for path in [
            "crates/ndaal-kv/clippy.toml",
            "crates/ndaal-kv/.clippy.toml",
            ".clippy.toml",
        ] {
            assert!(
                config_bypass(
                    path, "x = 1
"
                )
                .is_some(),
                "{path}"
            );
        }
    }

    #[test]
    fn member_manifests_must_inherit_workspace_lints() {
        let inherits = "[package]
name = \"x\"

[lints]
workspace = true
";
        let own = "[package]
name = \"x\"

[lints.clippy]
disallowed_methods = \"allow\"
";
        assert!(config_bypass("crates/ndaal-kv/Cargo.toml", inherits).is_none());
        assert!(config_bypass("xtask/Cargo.toml", inherits).is_none());
        assert!(config_bypass("crates/ndaal-kv/Cargo.toml", own).is_some());
        assert!(
            config_bypass("Cargo.toml", own).is_none(),
            "root manifest defines the lints"
        );
        assert!(
            config_bypass("tests/fixtures/deny-gpl/Cargo.toml", own).is_none(),
            "fixtures"
        );
    }

    #[test]
    fn cargo_config_cannot_touch_clippy() {
        let config = "[build]
rustflags = [\"-A\", \"clippy::disallowed_methods\"]
";
        assert!(config_bypass(".cargo/config.toml", config).is_some());
        assert!(config_bypass(".cargo/config", config).is_some());
        assert!(
            config_bypass(
                ".cargo/config.toml",
                "[build]
jobs = 4
"
            )
            .is_none()
        );
    }
}
