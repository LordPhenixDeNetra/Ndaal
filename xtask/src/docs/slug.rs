//! Heading anchors, computed as GitHub (and mdBook) do.

use std::collections::{BTreeMap, BTreeSet};

/// Anchor of one heading: the rendered text in lower case, without anything
/// but letters, digits, spaces, `-` and `_`, spaces turned into `-`.
pub fn slugify(heading: &str) -> String {
    rendered_text(heading)
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_'))
        .map(|c| if c == ' ' { '-' } else { c })
        .collect()
}

/// Every anchor a document defines: its headings (outside code blocks, a
/// repeated one suffixed `-1`, `-2`...) and explicit `<a id="...">`.
pub fn anchors(markdown: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for line in outside_code(markdown) {
        if let Some(text) = heading_text(line) {
            let base = slugify(text);
            let count = seen.entry(base.clone()).or_insert(0);
            let slug = if *count == 0 {
                base
            } else {
                format!("{base}-{count}")
            };
            *count += 1;
            found.insert(slug);
        }
        found.extend(explicit_ids(line));
    }
    found
}

/// The lines of `markdown` that are not inside a fenced code block.
pub fn outside_code(markdown: &str) -> impl Iterator<Item = &str> {
    let mut fence: Option<&str> = None;
    markdown.lines().filter(move |line| {
        let trimmed = line.trim_start();
        let marker = ["```", "~~~"].into_iter().find(|m| trimmed.starts_with(m));
        match (fence, marker) {
            (None, Some(m)) => {
                fence = Some(m);
                false
            }
            (Some(open), Some(m)) if open == m => {
                fence = None;
                false
            }
            (Some(_), _) => false,
            (None, None) => true,
        }
    })
}

/// The text of an ATX heading (`## Title ##`), if `line` is one.
fn heading_text(line: &str) -> Option<&str> {
    let hashes = line.chars().take_while(|c| *c == '#').count();
    if !(1..=6).contains(&hashes) {
        return None;
    }
    let rest = line[hashes..].strip_prefix(' ')?;
    Some(rest.trim().trim_end_matches('#').trim_end())
}

/// Heading text as rendered: code ticks, emphasis stars and link targets
/// removed (`[text](url)` keeps `text`).
fn rendered_text(heading: &str) -> String {
    let mut out = String::new();
    let mut rest = heading;
    while let Some(start) = rest.find('[') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find("](").and_then(|mid| {
            let close = after[mid + 2..].find(')')?;
            Some((mid, mid + 2 + close))
        }) {
            Some((mid, close)) => {
                out.push_str(&after[..mid]);
                rest = &after[close + 1..];
            }
            None => {
                out.push('[');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out.replace(['`', '*'], "")
}

/// Values of `id="..."` and `name="..."` in `<a ...>` tags of `line`.
fn explicit_ids(line: &str) -> Vec<String> {
    let mut ids = Vec::new();
    for tag in line.split("<a ").skip(1) {
        let tag = tag.split('>').next().unwrap_or_default();
        for attribute in ["id=\"", "name=\""] {
            if let Some(start) = tag.find(attribute) {
                let value = &tag[start + attribute.len()..];
                if let Some(end) = value.find('"') {
                    ids.push(value[..end].to_owned());
                }
            }
        }
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::{anchors, slugify};

    #[test]
    fn slug_matches_github() {
        let cases = [
            (
                "ADR-032 \u{2014} Frontal HTTP sur `axum`",
                "adr-032--frontal-http-sur-axum",
            ),
            ("14.3 HTTP et gRPC", "143-http-et-grpc"),
            (
                "R\u{e9}f\u{e9}rence de configuration",
                "r\u{e9}f\u{e9}rence-de-configuration",
            ),
            ("C++ / Rust!", "c--rust"),
            ("8. API d'administration", "8-api-dadministration"),
            ("Voir [ADR-003](DECISIONS.md) *vite*", "voir-adr-003-vite"),
            ("snake_case et tiret-court", "snake_case-et-tiret-court"),
        ];
        for (heading, slug) in cases {
            assert_eq!(slugify(heading), slug, "{heading}");
        }
    }

    #[test]
    fn duplicate_headings_get_suffixes() {
        let doc = "# Contexte\n## Contexte\n### Contexte\n";
        let expected = ["contexte", "contexte-1", "contexte-2"];
        assert_eq!(
            anchors(doc),
            expected.iter().map(|s| (*s).to_owned()).collect()
        );
    }

    #[test]
    fn headings_inside_code_blocks_are_ignored() {
        let doc = "# Vrai\n```bash\n# commentaire\n```\n~~~\n## Faux\n~~~\n## Encore vrai ##\n";
        let expected = ["encore-vrai", "vrai"];
        assert_eq!(
            anchors(doc),
            expected.iter().map(|s| (*s).to_owned()).collect()
        );
    }

    #[test]
    fn explicit_anchors_and_non_headings() {
        let doc = "<a id=\"ici\"></a> texte\n#pas-un-titre\n####### sept\n";
        assert_eq!(anchors(doc), ["ici".to_owned()].into_iter().collect());
    }
}
