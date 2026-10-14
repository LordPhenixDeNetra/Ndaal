//! Text rules: emoji, non-ASCII, whitespace, line endings, file length.

use std::path::Path;

use super::{Rule, Violation};

/// Line limit of an ordinary Rust source file (CLAUDE.md section 4.6).
const MAX_RUST_LINES: usize = 800;
/// Line limit of a Rust test file (CLAUDE.md section 4.6).
const MAX_RUST_TEST_LINES: usize = 1500;
/// Extensions restricted to ASCII (CLAUDE.md section 4.7).
const ASCII_ONLY: [&str; 4] = ["rs", "toml", "yml", "yaml"];
/// Fuzz inputs: their bytes are the test data (trailing spaces, missing
/// newlines, any character); only their names are checked.
const RAW_DATA_DIR: &str = "fuzz/corpus/";
/// Windows scripts, which require CRLF line endings.
const CRLF_ALLOWED: [&str; 3] = ["bat", "cmd", "ps1"];

/// Checks one file's name and contents against the text and length rules.
///
/// The contents of binary files (a NUL byte, or not valid UTF-8) are skipped;
/// their names are still checked.
pub fn check_file(path: &str, bytes: &[u8]) -> Vec<Violation> {
    let mut found = check_path(path);
    let raw_data = path.starts_with(RAW_DATA_DIR) && !path.ends_with(".md");
    if raw_data || bytes.contains(&0) {
        return found;
    }
    let Ok(text) = std::str::from_utf8(bytes) else {
        return found;
    };
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    for (index, raw) in text.split_inclusive('\n').enumerate() {
        check_line(path, ext, index + 1, raw, &mut found);
    }
    if !text.is_empty() && !text.ends_with('\n') {
        found.push(violation(
            path,
            None,
            Rule::MissingFinalNewline,
            "add a final newline",
        ));
    }
    if ext == "rs" {
        found.extend(super::rust::lint_bypass(path, text));
    }
    found.extend(super::rust::config_bypass(path, text));
    if let Some(limit) = rust_line_limit(path, ext) {
        let count = text.lines().count();
        if count > limit {
            let detail = format!("{count} lines, limit {limit}");
            found.push(violation(path, None, Rule::FileTooLong, &detail));
        }
    }
    found
}

fn check_line(path: &str, ext: &str, line: usize, raw: &str, found: &mut Vec<Violation>) {
    let content = raw.strip_suffix('\n').unwrap_or(raw);
    let (content, crlf) = match content.strip_suffix('\r') {
        Some(stripped) => (stripped, true),
        None => (content, false),
    };
    if crlf && !CRLF_ALLOWED.contains(&ext) {
        found.push(violation(
            path,
            Some(line),
            Rule::Crlf,
            "use LF line endings",
        ));
    }
    if content.ends_with([' ', '\t']) {
        found.push(violation(
            path,
            Some(line),
            Rule::TrailingWhitespace,
            "remove it",
        ));
    }
    if let Some(c) = content.chars().find(|&c| is_emoji(c)) {
        let detail = format!("U+{:04X}", u32::from(c));
        found.push(violation(path, Some(line), Rule::Emoji, &detail));
    }
    if ASCII_ONLY.contains(&ext)
        && let Some(c) = content.chars().find(|c| !c.is_ascii())
    {
        let detail = format!("U+{:04X}, use an escape or ASCII", u32::from(c));
        found.push(violation(path, Some(line), Rule::NonAscii, &detail));
    }
}

/// File names are identifiers: ASCII only, hence no emoji (CLAUDE.md section 4.7).
fn check_path(path: &str) -> Vec<Violation> {
    if let Some(c) = path.chars().find(|&c| is_emoji(c)) {
        let detail = format!("file name contains U+{:04X}", u32::from(c));
        return vec![violation(path, None, Rule::Emoji, &detail)];
    }
    if let Some(c) = path.chars().find(|c| !c.is_ascii()) {
        let detail = format!("file name contains U+{:04X}, use ASCII", u32::from(c));
        return vec![violation(path, None, Rule::NonAscii, &detail)];
    }
    Vec::new()
}

/// Emoji ranges of CLAUDE.md section 4.7.
fn is_emoji(c: char) -> bool {
    matches!(u32::from(c), 0x1F000..=0x1FAFF | 0x2600..=0x27BF | 0xFE0F)
}

/// Line limit for a Rust file, or `None` when the file is not limited.
fn rust_line_limit(path: &str, ext: &str) -> Option<usize> {
    let generated = path.ends_with("_generated.rs")
        || path.starts_with("generated/")
        || path.contains("/generated/");
    if ext != "rs" || generated {
        return None;
    }
    let test =
        path.starts_with("tests/") || path.contains("/tests/") || path.ends_with("_tests.rs");
    Some(if test {
        MAX_RUST_TEST_LINES
    } else {
        MAX_RUST_LINES
    })
}

fn violation(path: &str, line: Option<usize>, rule: Rule, detail: &str) -> Violation {
    Violation {
        path: path.to_owned(),
        line,
        rule,
        detail: detail.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::Rule;
    use super::check_file;

    fn rules(path: &str, content: &str) -> Vec<(Rule, Option<usize>)> {
        check_file(path, content.as_bytes())
            .into_iter()
            .map(|v| (v.rule, v.line))
            .collect()
    }

    #[test]
    fn emoji_in_markdown_is_reported() {
        assert_eq!(
            rules("README.md", "ok\nbad \u{1F600}\n"),
            vec![(Rule::Emoji, Some(2))]
        );
    }

    #[test]
    fn variation_selector_is_reported() {
        let found = rules("a.md", "\u{2764}\u{FE0F}\n");
        assert!(!found.is_empty());
        assert!(found.iter().all(|r| *r == (Rule::Emoji, Some(1))));
    }

    #[test]
    fn fuzz_corpus_contents_are_raw_data() {
        let input = "no final newline, trailing space \u{1F600} ";
        assert!(rules("fuzz/corpus/common_codec/seed-1", input).is_empty());
        assert!(!rules("fuzz/corpus/README.md", input).is_empty());
        assert!(!rules("fuzz/corpus-notes.txt", input).is_empty());
    }

    #[test]
    fn accents_allowed_in_markdown() {
        assert!(rules("a.md", "r\u{e9}sum\u{e9}\n").is_empty());
    }

    #[test]
    fn non_ascii_in_rust_is_reported() {
        assert_eq!(
            rules("a.rs", "// r\u{e9}sum\u{e9}\n"),
            vec![(Rule::NonAscii, Some(1))]
        );
    }

    #[test]
    fn non_ascii_in_toml_and_yaml_is_reported() {
        for path in ["a.toml", "a.yml", "a.yaml"] {
            assert_eq!(
                rules(path, "k = \"\u{e9}\"\n"),
                vec![(Rule::NonAscii, Some(1))],
                "{path}"
            );
        }
    }

    #[test]
    fn trailing_whitespace_is_reported() {
        assert_eq!(
            rules("a.rs", "fn x() {} \n"),
            vec![(Rule::TrailingWhitespace, Some(1))]
        );
    }

    #[test]
    fn crlf_is_reported_except_windows_scripts() {
        assert_eq!(rules("a.md", "a\r\n"), vec![(Rule::Crlf, Some(1))]);
        for path in ["a.bat", "a.cmd", "a.ps1"] {
            assert!(rules(path, "a\r\n").is_empty(), "{path}");
        }
    }

    #[test]
    fn missing_final_newline_is_reported() {
        assert_eq!(
            rules("a.rs", "fn x() {}"),
            vec![(Rule::MissingFinalNewline, None)]
        );
    }

    #[test]
    fn empty_file_is_accepted() {
        assert!(rules("a.rs", "").is_empty());
    }

    #[test]
    fn long_rust_file_is_reported() {
        assert_eq!(
            rules("src/a.rs", &"//\n".repeat(801)),
            vec![(Rule::FileTooLong, None)]
        );
    }

    #[test]
    fn emoji_in_file_name_is_reported() {
        assert!(rules("docs/note_\u{1F600}.md", "ok\n").contains(&(Rule::Emoji, None)));
    }

    #[test]
    fn non_ascii_file_name_is_reported() {
        assert_eq!(
            rules("docs/r\u{e9}sum\u{e9}.md", "ok\n"),
            vec![(Rule::NonAscii, None)]
        );
    }

    #[test]
    fn rust_file_at_limit_passes() {
        assert!(rules("src/a.rs", &"//\n".repeat(800)).is_empty());
    }

    #[test]
    fn test_files_have_higher_limit() {
        assert!(rules("tests/a.rs", &"//\n".repeat(1500)).is_empty());
        assert!(rules("src/a_tests.rs", &"//\n".repeat(1500)).is_empty());
        assert_eq!(
            rules("tests/a.rs", &"//\n".repeat(1501)),
            vec![(Rule::FileTooLong, None)]
        );
    }

    #[test]
    fn generated_files_are_exempt() {
        assert!(rules("src/x_generated.rs", &"//\n".repeat(5000)).is_empty());
        assert!(rules("src/generated/y.rs", &"//\n".repeat(5000)).is_empty());
    }

    #[test]
    fn binary_files_are_skipped() {
        assert!(check_file("a.png", &[0x89, 0x50, 0x00, 0xFF]).is_empty());
        assert!(check_file("a.bin", &[0xFF, 0xFE, 0x41]).is_empty());
    }
}
