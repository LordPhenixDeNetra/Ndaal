//! Fluent translations and language selection for the CLI and console
//! (ADR-029, ENF-I18N-01 to 06).
//!
//! The `.ftl` files of `locales/<lang>/` are embedded in the binary: `ndaal`
//! stays a single file. Only messages for humans are translated, never logs,
//! metrics, protocol errors or configuration names (ENF-I18N-02).

#![forbid(unsafe_code)]

mod lang;

use fluent_bundle::{FluentArgs, FluentBundle, FluentResource};
use unic_langid::LanguageIdentifier;

pub use lang::choose;

/// The translations of one language.
#[derive(Debug)]
pub struct Locale {
    /// Language code: `en`, `fr`.
    pub code: &'static str,
    /// Embedded `.ftl` files: `(name without extension, contents)`.
    pub files: &'static [(&'static str, &'static str)],
}

/// Every shipped language; English first. A new language is a new
/// `locales/<code>/` folder and one entry here: a test fails if they differ.
pub const LOCALES: &[Locale] = &[
    Locale {
        code: "en",
        files: &[("cli", include_str!("../../../locales/en/cli.ftl"))],
    },
    Locale {
        code: "fr",
        files: &[("cli", include_str!("../../../locales/fr/cli.ftl"))],
    },
];

/// English: the reference language and the fallback for missing keys.
pub const EN: &Locale = &LOCALES[0];

/// Formats messages in one language, falling back to English.
pub struct Translator {
    locale: &'static Locale,
    bundle: FluentBundle<FluentResource>,
    fallback: FluentBundle<FluentResource>,
}

impl std::fmt::Debug for Translator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Translator")
            .field("lang", &self.locale.code)
            .finish_non_exhaustive()
    }
}

fn bundle<'a>(
    code: &str,
    sources: impl IntoIterator<Item = &'a str>,
) -> FluentBundle<FluentResource> {
    let id: LanguageIdentifier = code.parse().unwrap_or_default();
    let mut bundle = FluentBundle::new(vec![id]);
    // Unicode isolation marks around variables would show up in terminals.
    bundle.set_use_isolating(false);
    for source in sources {
        // The embedded files are checked by the tests (no syntax error, same
        // keys in every language): a bad entry is skipped, never a panic.
        let resource = FluentResource::try_new(source.to_owned()).unwrap_or_else(|(r, _)| r);
        let _ = bundle.add_resource(resource);
    }
    bundle
}

impl Translator {
    /// Messages of `locale`.
    pub fn new(locale: &'static Locale) -> Self {
        Self::from_sources(
            locale,
            locale.files.iter().map(|(_, text)| *text),
            EN.files.iter().map(|(_, text)| *text),
        )
    }

    fn from_sources<'a>(
        locale: &'static Locale,
        sources: impl IntoIterator<Item = &'a str>,
        fallback: impl IntoIterator<Item = &'a str>,
    ) -> Self {
        Self {
            locale,
            bundle: bundle(locale.code, sources),
            fallback: bundle(EN.code, fallback),
        }
    }

    /// The language of the messages.
    pub fn locale(&self) -> &'static Locale {
        self.locale
    }

    /// The message `key`, without variables.
    pub fn t(&self, key: &str) -> String {
        self.t_args(key, &[])
    }

    /// The message `key` with its variables. A key missing in this language
    /// gives the English text; missing everywhere, the key itself.
    pub fn t_args(&self, key: &str, args: &[(&str, &str)]) -> String {
        let mut fluent_args = FluentArgs::new();
        for (name, value) in args {
            fluent_args.set(*name, (*value).to_owned());
        }
        format(&self.bundle, key, &fluent_args)
            .or_else(|| format(&self.fallback, key, &fluent_args))
            .unwrap_or_else(|| key.to_owned())
    }
}

fn format(
    bundle: &FluentBundle<FluentResource>,
    key: &str,
    args: &FluentArgs<'_>,
) -> Option<String> {
    let pattern = bundle.get_message(key)?.value()?;
    let mut errors = Vec::new();
    Some(
        bundle
            .format_pattern(pattern, Some(args), &mut errors)
            .into_owned(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const FR: &Locale = &LOCALES[1];

    #[test]
    fn missing_keys_fall_back_to_english_then_to_the_key() {
        let t = Translator::from_sources(FR, ["a = un\n"], ["a = one\nb = two\n"]);
        assert_eq!(t.t("a"), "un");
        assert_eq!(t.t("b"), "two");
        assert_eq!(t.t("c"), "c");
    }

    #[test]
    fn variables_are_substituted_without_isolation_marks() {
        let t = Translator::new(EN);
        assert_eq!(
            t.t_args("cli-not-available", &[("command", "admin")]),
            "The command admin is not available yet."
        );
    }
}
