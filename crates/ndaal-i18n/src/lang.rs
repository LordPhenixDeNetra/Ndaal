//! Choice of the message language (ENF-I18N-03).

use crate::{EN, LOCALES, Locale};

/// The shipped locale named by `value`: `fr`, `fr-FR`, `fr_FR.UTF-8`. `auto`,
/// an empty value or an unknown language give `None`.
fn find(value: &str) -> Option<&'static Locale> {
    let primary = value
        .split(['-', '_', '.', '@'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    LOCALES.iter().find(|locale| locale.code == primary)
}

/// The first recognized language among, in priority order: `--lang`,
/// `NDAAL_LANG`, `node.lang`, the system language; English otherwise.
/// An unknown value is skipped, never an error: the messages must stay
/// readable whatever the environment.
pub fn choose(
    flag: Option<&str>,
    env: Option<&str>,
    config: Option<&str>,
    system: Option<&str>,
) -> &'static Locale {
    [flag, env, config, system]
        .into_iter()
        .flatten()
        .find_map(find)
        .unwrap_or(EN)
}
