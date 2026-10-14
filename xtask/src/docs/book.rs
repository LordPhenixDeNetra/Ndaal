//! Sources of the mdBook site, generated from the documents at the root
//! (they stay there: every existing link keeps working on GitHub).

use super::links;

/// Repository URL of files that are not pages of the book.
pub const GITHUB_BLOB: &str = "https://github.com/LordPhenixDeNetra/Ndaal/blob/main/";

/// The pages, in order: (document at the root, chapter title). `README.md`
/// becomes `introduction.md`: mdBook would turn it into `index.html` without
/// fixing the links to it.
pub const BOOK_PAGES: &[(&str, &str)] = &[
    ("README.md", "Introduction"),
    ("CAHIER_DES_CHARGES.md", "Cahier des charges"),
    ("ARCHITECTURE.md", "Architecture"),
    ("DECISIONS.md", "D\u{e9}cisions d'architecture"),
    ("FORMATS_DE_DONNEES.md", "Formats de donn\u{e9}es"),
    ("API_ET_PROTOCOLES.md", "API et protocoles"),
    ("EXPLOITATION.md", "Exploitation"),
    ("STRATEGIE_DE_TESTS.md", "Strat\u{e9}gie de tests"),
    ("CAHIER_DES_TACHES.md", "Cahier des t\u{e2}ches"),
    ("POINTS_REPORTES.md", "Points report\u{e9}s"),
    ("GLOSSAIRE.md", "Glossaire"),
    ("CONTRIBUTING.md", "Contribuer"),
    ("CLAUDE.md", "R\u{e8}gles de code"),
    ("SECURITY.md", "S\u{e9}curit\u{e9}"),
    ("CODE_OF_CONDUCT.md", "Code de conduite"),
];

/// File name of a page inside the book.
pub fn book_name(document: &str) -> &str {
    if document == "README.md" {
        "introduction.md"
    } else {
        document
    }
}

/// `SUMMARY.md`: the introduction as a prefix chapter, then every page.
pub fn summary() -> String {
    let mut out = String::from("# Sommaire\n\n");
    for (i, (document, title)) in BOOK_PAGES.iter().enumerate() {
        let entry = format!("[{title}]({})\n", book_name(document));
        if i == 0 {
            out.push_str(&entry);
            out.push('\n');
        } else {
            out.push_str(&format!("- {entry}"));
        }
    }
    out
}

/// `markdown` of the root document `document` with every relative link to
/// something that is not a page rewritten: to its GitHub URL, or to the
/// page's book name.
pub fn rewrite_links(document: &str, markdown: &str) -> String {
    let all = links::links(markdown);
    let mut out = String::with_capacity(markdown.len());
    for (index, line) in markdown.split_inclusive('\n').enumerate() {
        let number = index + 1;
        let mut line = line.to_owned();
        for link in all.iter().filter(|l| l.line == number) {
            if let Some(new) = new_target(document, link.target) {
                line = line.replacen(&format!("]({})", link.target), &format!("]({new})"), 1);
                line = line.replacen(&format!("]: {}", link.target), &format!("]: {new}"), 1);
            }
        }
        out.push_str(&line);
    }
    out
}

/// Where `target` must point in the book, or `None` to keep it.
fn new_target(document: &str, target: &str) -> Option<String> {
    if links::is_external(target) || target.starts_with('#') {
        return None;
    }
    let (path, anchor) = match target.split_once('#') {
        Some((path, anchor)) => (path, format!("#{anchor}")),
        None => (target, String::new()),
    };
    let resolved = links::resolve(document, path)?;
    match BOOK_PAGES.iter().find(|(page, _)| *page == resolved) {
        Some((page, _)) if *page == "README.md" => Some(format!("introduction.md{anchor}")),
        Some(_) => None,
        None => Some(format!("{GITHUB_BLOB}{resolved}{anchor}")),
    }
}

#[cfg(test)]
mod tests {
    use super::{BOOK_PAGES, rewrite_links, summary};

    #[test]
    fn links_outside_the_book_point_to_github() {
        let doc = "[a](ARCHITECTURE.md#3-organisation) [d](deploy/docker/Dockerfile)\n\
                   [r](README.md#ndaal) [w](https://x.org) [s](#ici) [b](bench/)\n\
                   `[c](deploy/)`\n";
        let expected = "[a](ARCHITECTURE.md#3-organisation) \
                        [d](https://github.com/LordPhenixDeNetra/Ndaal/blob/main/deploy/docker/Dockerfile)\n\
                        [r](introduction.md#ndaal) [w](https://x.org) [s](#ici) \
                        [b](https://github.com/LordPhenixDeNetra/Ndaal/blob/main/bench)\n\
                        `[c](deploy/)`\n";
        assert_eq!(rewrite_links("EXPLOITATION.md", doc), expected);
    }

    #[test]
    fn summary_lists_every_page_in_order() {
        let text = summary();
        assert!(text.starts_with("# Sommaire\n\n[Introduction](introduction.md)\n\n"));
        let mut last = 0;
        for (document, title) in &BOOK_PAGES[1..] {
            let entry = format!("- [{title}]({document})\n");
            let at = text.find(&entry).expect(document);
            assert!(at > last, "{document} out of order");
            last = at;
        }
    }
}
