//! Relative links of the Markdown files: target file and anchor must exist.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use super::slug;

/// A link whose target does not exist.
#[derive(Debug, PartialEq, Eq)]
pub struct BrokenLink {
    /// File holding the link.
    pub file: String,
    /// 1-based line.
    pub line: usize,
    /// The link target as written.
    pub target: String,
    /// What is missing.
    pub reason: String,
}

impl fmt::Display for BrokenLink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}: {} ({})",
            self.file, self.line, self.target, self.reason
        )
    }
}

/// One link: its line and target as written.
pub struct Link<'a> {
    pub line: usize,
    pub target: &'a str,
}

/// Inline links `[text](target)` and `![alt](target)`, and reference
/// definitions `[name]: target`, outside code blocks and code spans.
pub fn links(markdown: &str) -> Vec<Link<'_>> {
    let mut found = Vec::new();
    let mut fence: Option<&str> = None;
    for (index, line) in markdown.lines().enumerate() {
        let trimmed = line.trim_start();
        if let Some(m) = ["```", "~~~"].into_iter().find(|m| trimmed.starts_with(m)) {
            fence = match fence {
                None => Some(m),
                Some(open) if open == m => None,
                other => other,
            };
            continue;
        }
        if fence.is_some() {
            continue;
        }
        let number = index + 1;
        if let Some(target) = reference_definition(trimmed) {
            found.push(Link {
                line: number,
                target,
            });
            continue;
        }
        for target in inline_targets(line) {
            found.push(Link {
                line: number,
                target,
            });
        }
    }
    found
}

/// The target of `[name]: target`, if `line` is a reference definition.
fn reference_definition(line: &str) -> Option<&str> {
    let rest = line.strip_prefix('[')?;
    let close = rest.find("]: ")?;
    rest[close + 3..].split_whitespace().next()
}

/// Targets of the inline links of `line`, skipping code spans.
fn inline_targets(line: &str) -> Vec<&str> {
    let mut targets = Vec::new();
    for (i, segment) in line.split('`').enumerate() {
        // Odd segments are inside a code span.
        if i % 2 == 1 {
            continue;
        }
        let mut rest = segment;
        while let Some(start) = rest.find("](") {
            let after = &rest[start + 2..];
            let end = after.find(')').unwrap_or(after.len());
            let target = after[..end].split_whitespace().next().unwrap_or_default();
            let target = target.trim_start_matches('<').trim_end_matches('>');
            if !target.is_empty() {
                targets.push(target);
            }
            rest = &after[end..];
        }
    }
    targets
}

/// Whether `target` leaves the repository (scheme, protocol-relative).
pub fn is_external(target: &str) -> bool {
    target.contains("://") || target.starts_with("mailto:") || target.starts_with("//")
}

/// `target` (without its anchor) resolved from the directory of `file`, as a
/// repository path; `None` when it climbs above the root.
pub fn resolve(file: &str, target: &str) -> Option<String> {
    let mut parts: Vec<&str> = file.split('/').collect();
    parts.pop();
    for part in target.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            other => parts.push(other),
        }
    }
    Some(parts.join("/"))
}

/// Every broken relative link of `files` (path, contents). A target is a
/// listed file or a directory holding one; a `.md` target's anchor must be
/// one of its headings.
pub fn check(files: &[(String, String)]) -> Vec<BrokenLink> {
    let contents: BTreeMap<&str, &str> = files
        .iter()
        .map(|(path, text)| (path.as_str(), text.as_str()))
        .collect();
    let mut anchor_cache: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut broken = Vec::new();
    for (file, text) in files.iter().filter(|(path, _)| path.ends_with(".md")) {
        for link in links(text) {
            if is_external(link.target) {
                continue;
            }
            if let Some(reason) = check_one(file, link.target, &contents, &mut anchor_cache) {
                broken.push(BrokenLink {
                    file: file.clone(),
                    line: link.line,
                    target: link.target.to_owned(),
                    reason: reason.to_owned(),
                });
            }
        }
    }
    broken
}

/// Why `target`, written in `file`, is broken; `None` when it resolves.
fn check_one(
    file: &str,
    target: &str,
    contents: &BTreeMap<&str, &str>,
    anchor_cache: &mut BTreeMap<String, BTreeSet<String>>,
) -> Option<&'static str> {
    let (path, anchor) = match target.split_once('#') {
        Some((path, anchor)) => (path, Some(anchor)),
        None => (target, None),
    };
    let resolved = if path.is_empty() {
        file.to_owned()
    } else {
        match resolve(file, path) {
            Some(resolved) => resolved,
            None => return Some("outside the repository"),
        }
    };
    let is_file = contents.contains_key(resolved.as_str());
    let prefix = format!("{}/", resolved.trim_end_matches('/'));
    let is_dir = contents.keys().any(|p| p.starts_with(&prefix));
    if !is_file && !is_dir {
        return Some("no such file");
    }
    let anchor = anchor.filter(|_| is_file && resolved.ends_with(".md"))?;
    let target_text = contents[resolved.as_str()];
    let known = anchor_cache
        .entry(resolved)
        .or_insert_with(|| slug::anchors(target_text));
    (!known.contains(&percent_decode(anchor))).then_some("no such heading")
}

/// `%C3%A9` back to the character; text that is not valid UTF-8 once
/// decoded is kept as written.
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes
            .get(i + 1..i + 3)
            .and_then(|h| std::str::from_utf8(h).ok());
        match (bytes[i], hex.and_then(|h| u8::from_str_radix(h, 16).ok())) {
            (b'%', Some(byte)) => {
                out.push(byte);
                i += 3;
            }
            (byte, _) => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8(out).unwrap_or_else(|_| text.to_owned())
}

#[cfg(test)]
mod tests {
    use super::check;

    fn reasons(list: &[(&str, &str)]) -> Vec<(usize, String, String)> {
        let files: Vec<(String, String)> = list
            .iter()
            .map(|(p, t)| ((*p).to_owned(), (*t).to_owned()))
            .collect();
        check(&files)
            .into_iter()
            .map(|b| (b.line, b.target, b.reason))
            .collect()
    }

    fn expected(list: &[(usize, &str, &str)]) -> Vec<(usize, String, String)> {
        list.iter()
            .map(|(l, t, r)| (*l, (*t).to_owned(), (*r).to_owned()))
            .collect()
    }

    #[test]
    fn broken_file_and_anchor_are_reported() {
        let a = "# Titre\n[ok](B.md#d\u{e9}cision) [ok](#titre)\n\
                 [x](C.md) [y](B.md#absent)\n[z](../hors.md)\n";
        let found = reasons(&[("A.md", a), ("B.md", "## D\u{e9}cision\n")]);
        let wanted = expected(&[
            (3, "C.md", "no such file"),
            (3, "B.md#absent", "no such heading"),
            (4, "../hors.md", "outside the repository"),
        ]);
        assert_eq!(found, wanted);
    }

    #[test]
    fn external_and_code_links_are_skipped() {
        let doc = "[w](https://x.org/a.md) [m](mailto:a@b.c) `[c](absent.md)`\n\
                   ```\n[d](absent.md)\n```\n[ref]: https://x.org\n";
        assert!(reasons(&[("A.md", doc)]).is_empty());
    }

    #[test]
    fn directories_files_and_encoded_anchors() {
        let a = "[d](../deploy/) [f](../deploy/docker/Dockerfile) \
                 [e](../B.md#d%C3%A9cision)\n[r]: ../absent.txt\n";
        let found = reasons(&[
            ("docs/A.md", a),
            ("deploy/docker/Dockerfile", "FROM x\n"),
            ("B.md", "# D\u{e9}cision\n"),
        ]);
        assert_eq!(found, expected(&[(2, "../absent.txt", "no such file")]));
    }
}
