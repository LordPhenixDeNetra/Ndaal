//! The real `ndaal` binary: commands, configuration sources and languages
//! (T01.06, T01.12, T01.23).

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use ndaal_io::real::RealDisk;
use ndaal_io::{Disk, File, OpenOptions};

const COMMANDS: [&str; 7] = [
    "server", "admin", "backup", "restore", "bench", "config", "doctor",
];

/// `ndaal args...` with a clean language environment: `NDAAL_LANG` set to
/// `lang` (or removed), and no `NDAAL_*` configuration variable.
fn ndaal(args: &[&str], lang: Option<&str>, env: &[(&str, &str)]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ndaal"));
    command.args(args);
    for (name, _) in std::env::vars() {
        if name.starts_with("NDAAL_") {
            command.env_remove(name);
        }
    }
    if let Some(lang) = lang {
        command.env("NDAAL_LANG", lang);
    }
    command.envs(env.iter().copied());
    command.output().expect("run ndaal")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("utf-8 stdout")
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("utf-8 stderr")
}

fn code(output: &Output) -> Option<i32> {
    output.status.code()
}

/// A configuration file with `text` in a fresh temporary directory.
fn config_file(text: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("ndaal.toml");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("runtime");
    runtime.block_on(async {
        let create = OpenOptions {
            create: true,
            ..OpenOptions::default()
        };
        let file = RealDisk.open(&path, create).await.expect("create");
        file.write_all_at(text.as_bytes(), 0).await.expect("write");
    });
    (dir, path)
}

fn path_arg(path: &Path) -> &str {
    path.to_str().expect("utf-8 temp path")
}

#[test]
fn help_lists_every_command() {
    let output = ndaal(&["--help"], Some("en"), &[]);
    assert_eq!(code(&output), Some(0), "{}", stderr(&output));
    let help = stdout(&output);
    for command in COMMANDS {
        assert!(help.contains(command), "{command} missing:\n{help}");
    }
    assert!(
        help.contains("Commands") && help.contains("Start a node"),
        "{help}"
    );
}

#[test]
fn help_is_translated_with_lang() {
    let help = stdout(&ndaal(&["--lang", "fr", "--help"], Some("en"), &[]));
    assert!(help.contains("Commandes"), "{help}");
    assert!(help.contains("D\u{e9}marrer un n\u{153}ud"), "{help}");
    assert!(help.contains("Utilisation"), "{help}");
    let sub = stdout(&ndaal(&["config", "--help", "--lang=fr"], None, &[]));
    assert!(sub.contains("Afficher la configuration effective"), "{sub}");
}

#[test]
fn unknown_lang_falls_back_without_error() {
    let output = ndaal(&["--lang", "xx", "--help"], Some("en"), &[]);
    assert_eq!(code(&output), Some(0), "{}", stderr(&output));
    assert!(stdout(&output).contains("Start a node"));
}

#[test]
fn node_lang_from_the_configuration_file_selects_the_language() {
    // Both languages: whatever the system language, one of them differs.
    for (lang, heading) in [("fr", "Commandes:"), ("en", "Commands:")] {
        let (_dir, path) = config_file(&format!("[node]\nlang = \"{lang}\"\n"));
        let help = stdout(&ndaal(&["--config", path_arg(&path), "--help"], None, &[]));
        assert!(help.contains(heading), "{lang}: {help}");
    }
}

#[test]
fn config_reference_lists_the_parameters() {
    let output = ndaal(&["config", "reference"], Some("en"), &[]);
    assert_eq!(code(&output), Some(0));
    let text = stdout(&output);
    assert!(
        text.contains("node.data_dir") && text.contains("NDAAL_NODE__DATA_DIR"),
        "{text}"
    );
}

#[test]
fn command_line_beats_environment_beats_file() {
    let (_dir, path) = config_file("[node]\ndata_dir = \"/file\"\nname = \"from-file\"\n");
    let config = path_arg(&path);
    let env = [("NDAAL_NODE__DATA_DIR", "/env")];
    let show = |args: &[&str], env: &[(&str, &str)]| stdout(&ndaal(args, Some("en"), env));

    let file_only = show(&["--config", config, "config", "show"], &[]);
    assert!(file_only.contains("data_dir = \"/file\""), "{file_only}");
    let with_env = show(&["--config", config, "config", "show"], &env);
    assert!(with_env.contains("data_dir = \"/env\""), "{with_env}");
    assert!(with_env.contains("name = \"from-file\""), "{with_env}");
    let with_cli = show(
        &["--config", config, "config", "show", "--data-dir", "/cli"],
        &env,
    );
    assert!(with_cli.contains("data_dir = \"/cli\""), "{with_cli}");
    let with_set = show(&["config", "show", "--set", "node.name=n7"], &env);
    assert!(with_set.contains("name = \"n7\""), "{with_set}");
}

#[test]
fn configuration_errors_exit_with_code_2_and_name_the_key() {
    let (_dir, path) = config_file("[node]\ndata_dirr = \"/x\"\n");
    let cases: [(Vec<&str>, &str); 4] = [
        (
            vec!["--config", path_arg(&path), "config", "show"],
            "node.data_dirr",
        ),
        (vec!["--set", "log.level=loud", "server"], "log.level"),
        (vec!["--set", "nonsense", "config", "show"], "nonsense"),
        (
            vec!["--config", "missing-ndaal.toml", "config", "show"],
            "missing-ndaal.toml",
        ),
    ];
    for (args, needle) in cases {
        let output = ndaal(&args, Some("en"), &[]);
        assert_eq!(code(&output), Some(2), "{args:?}");
        assert!(
            stderr(&output).contains(needle),
            "{args:?}: {}",
            stderr(&output)
        );
    }
}

#[test]
fn runtime_values_are_explained_in_french() {
    for (set, expected) in [
        ("runtime.cores.shard=x", "auto ou un nombre"),
        ("runtime.idle_spin=1h", "une dur\u{e9}e"),
    ] {
        let output = ndaal(&["--set", set, "config", "show"], Some("fr"), &[]);
        let text = stderr(&output);
        assert_eq!(code(&output), Some(2), "{set}");
        assert!(text.contains(expected), "{set}: {text}");
    }
}

#[test]
fn configuration_errors_are_translated() {
    let output = ndaal(
        &["--set", "log.level=loud", "config", "show"],
        Some("fr"),
        &[],
    );
    let text = stderr(&output);
    assert_eq!(code(&output), Some(2));
    assert!(text.contains("Valeur invalide pour log.level"), "{text}");
    assert!(text.contains("ligne de commande"), "{text}");
    assert!(text.contains("un niveau"), "{text}");
    assert!(
        !text.contains("invalid value") && !text.contains("loud"),
        "{text}"
    );
    let output = ndaal(&["--set", "a.b=1", "config", "show"], Some("fr"), &[]);
    assert!(stderr(&output).contains("Cl\u{e9} de configuration inconnue"));
}

#[test]
fn oversized_configuration_files_are_refused() {
    let (_dir, path) = config_file(&format!("{}\n", "#".repeat(1 << 20)));
    let output = ndaal(
        &["--config", path_arg(&path), "config", "show"],
        Some("fr"),
        &[],
    );
    let text = stderr(&output);
    assert_eq!(code(&output), Some(2), "{text}");
    assert!(text.contains("1048576"), "{text}");
    assert!(!text.contains("bytes"), "{text}");
}

/// A pipe has no length: reading it by its metadata would see an empty file
/// and silently start with the defaults.
#[cfg(unix)]
#[test]
fn a_pipe_is_never_read_as_an_empty_configuration() {
    use std::io::Write;
    use std::process::Stdio;

    let mut child = Command::new(env!("CARGO_BIN_EXE_ndaal"))
        .args(["--config", "/dev/stdin", "config", "show"])
        .env("NDAAL_LANG", "en")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    let mut stdin = child.stdin.take().expect("stdin");
    let _ = stdin.write_all(b"[node]\nname = \"piped\"\n");
    drop(stdin);
    let output = child.wait_with_output().expect("wait");
    let shown = stdout(&output);
    assert!(
        shown.contains("piped") || code(&output) == Some(2),
        "defaults used silently: {shown}"
    );
}

#[test]
fn public_listen_is_refused_without_allow_insecure() {
    let args = [
        "--lang",
        "fr",
        "--set",
        "http.listen=0.0.0.0:1",
        "config",
        "show",
    ];
    let output = ndaal(&args, None, &[]);
    assert_eq!(code(&output), Some(2));
    assert!(
        stderr(&output).contains("auth.allow_insecure"),
        "{}",
        stderr(&output)
    );
}

/// Containers set the file once in the environment, for `server` and for
/// the `health` probe alike (EXPLOITATION.md section 1.2).
#[test]
fn ndaal_config_names_the_configuration_file() {
    let (_dir, path) = config_file(
        "[node]
name = \"from-env-file\"
",
    );
    let (_other_dir, other) = config_file(
        "[node]
name = \"from-flag\"
",
    );
    let env = [("NDAAL_CONFIG", path_arg(&path))];
    let shown = stdout(&ndaal(&["config", "show"], Some("en"), &env));
    assert!(shown.contains("name = \"from-env-file\""), "{shown}");
    let args = ["--config", path_arg(&other), "config", "show"];
    let shown = stdout(&ndaal(&args, Some("en"), &env));
    assert!(shown.contains("name = \"from-flag\""), "{shown}");
    let missing = [("NDAAL_CONFIG", "missing-ndaal.toml")];
    let output = ndaal(&["config", "show"], Some("en"), &missing);
    assert_eq!(code(&output), Some(2));
}

#[test]
fn unbuilt_commands_say_so() {
    for command in ["admin", "backup", "restore", "bench", "doctor"] {
        let output = ndaal(&[command], Some("en"), &[]);
        assert_eq!(code(&output), Some(2), "{command}");
        assert!(stderr(&output).contains("not available yet"), "{command}");
    }
    let output = ndaal(&["admin"], Some("fr"), &[]);
    assert!(stderr(&output).contains("pas encore disponible"));
}

#[test]
fn usage_errors_are_translated() {
    let cases = [
        (&["frobnicate"][..], "Commande inconnue : frobnicate"),
        (&["--no-such-option"], "Option inconnue : --no-such-option"),
        (&["config", "show", "--lang"], "--lang"),
    ];
    for (args, needle) in cases {
        let output = ndaal(args, Some("fr"), &[]);
        let text = stderr(&output);
        assert_eq!(code(&output), Some(2), "{args:?}");
        assert!(text.contains(needle), "{args:?}: {text}");
        assert!(text.contains("ndaal --help"), "{args:?}: {text}");
        assert!(!text.contains("error:"), "{args:?}: {text}");
    }
}

#[test]
fn usage_errors_exit_with_code_2() {
    for args in [&["frobnicate"][..], &["--no-such-option"], &[]] {
        assert_eq!(code(&ndaal(args, Some("en"), &[])), Some(2), "{args:?}");
    }
}
