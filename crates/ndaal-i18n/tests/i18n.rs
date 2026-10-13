//! Language choice and completeness of the translations (T01.23).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use ndaal_i18n::{LOCALES, choose};
use ndaal_io::Disk;
use ndaal_io::real::RealDisk;

#[test]
fn language_priority_follows_enf_i18n_03() {
    let cases = [
        ((Some("fr"), Some("en"), Some("en"), Some("en_US")), "fr"),
        ((None, Some("fr"), Some("en"), Some("en_US")), "fr"),
        ((None, None, Some("fr"), Some("en_US")), "fr"),
        ((None, None, Some("auto"), Some("fr_FR.UTF-8")), "fr"),
        ((None, None, None, Some("fr-CA")), "fr"),
        ((Some("xx"), None, None, Some("fr")), "fr"),
        ((Some("FR"), None, None, None), "fr"),
        ((Some("xx"), Some(""), Some("auto"), Some("de_DE")), "en"),
        ((None, None, None, None), "en"),
    ];
    for ((flag, env, config, system), expected) in cases {
        let got = choose(flag, env, config, system).code;
        assert_eq!(got, expected, "{flag:?} {env:?} {config:?} {system:?}");
    }
}

/// Message and term identifiers of an `.ftl` text; a syntax error fails.
fn keys(label: &str, text: &str) -> BTreeSet<String> {
    let resource = fluent_syntax::parser::parse(text)
        .unwrap_or_else(|(_, errors)| panic!("{label}: {errors:?}"));
    resource
        .body
        .iter()
        .filter_map(|entry| match entry {
            fluent_syntax::ast::Entry::Message(m) => Some(m.id.name.to_owned()),
            fluent_syntax::ast::Entry::Term(t) => Some(format!("-{}", t.id.name)),
            _ => None,
        })
        .collect()
}

#[test]
fn every_language_has_every_key() {
    let reference = &LOCALES[0];
    for locale in LOCALES {
        let names: Vec<_> = locale.files.iter().map(|(n, _)| *n).collect();
        let expected: Vec<_> = reference.files.iter().map(|(n, _)| *n).collect();
        assert_eq!(names, expected, "{}: files", locale.code);
        for ((name, text), (_, en)) in locale.files.iter().zip(reference.files) {
            let label = format!("{}/{name}.ftl", locale.code);
            assert_eq!(
                keys(&label, text),
                keys("en", en),
                "{label}: keys differ from en"
            );
        }
    }
}

fn file_names(paths: &[PathBuf]) -> Vec<String> {
    let mut names: Vec<String> = paths
        .iter()
        .filter_map(|p| p.file_name()?.to_str().map(str::to_owned))
        .collect();
    names.sort();
    names
}

#[test]
fn embedded_locales_match_the_locales_folder() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("locales");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("runtime");
    runtime.block_on(async {
        let disk = RealDisk;
        let dirs = file_names(&disk.list_dir(&root).await.expect("list locales"));
        let mut codes: Vec<String> = LOCALES.iter().map(|l| l.code.to_owned()).collect();
        codes.sort();
        assert_eq!(dirs, codes, "locales/ folders and LOCALES differ");
        for locale in LOCALES {
            let listed = disk.list_dir(&root.join(locale.code)).await.expect("list");
            let mut embedded: Vec<String> = locale
                .files
                .iter()
                .map(|(n, _)| format!("{n}.ftl"))
                .collect();
            embedded.sort();
            assert_eq!(file_names(&listed), embedded, "locales/{}", locale.code);
        }
    });
}
