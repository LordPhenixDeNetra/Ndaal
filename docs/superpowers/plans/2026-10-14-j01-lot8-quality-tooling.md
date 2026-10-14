# J01 lot 8 — Outillage qualité : fuzzing, bancs, site de documentation — plan d'implémentation

> **Pour les agents :** sous-compétence requise : superpowers:executing-plans (ou subagent-driven-development), tâche par tâche.
>
> **Pas d'étape de commit** : le responsable commite sur une branche `j01-lot8-quality-tooling` et ouvre une demande de fusion ; la CI fait foi. Le compte rendu donne les commandes git **et le contenu du formulaire de demande de fusion**.

**Objectif :**
- une cible de fuzzing tourne chaque nuit, et un plantage ouvre un ticket (T01.14) ;
- un micro-banc `criterion` est mesuré chaque nuit et tracé dans le temps (T01.15) ;
- la documentation est publiée en site mdBook, construit en CI, avec des liens internes vérifiés automatiquement (T01.17).

**Architecture :**
- **Fuzzing.** La logique de chaque cible vit dans une crate du workspace, `ndaal-fuzz`. Elle compile en Rust stable et est donc testée sur les trois systèmes : le corpus versionné y est rejoué à chaque demande de fusion. Le dossier `fuzz/` est un workspace `cargo fuzz` séparé, dont chaque cible appelle `ndaal-fuzz`. Il n'est construit qu'avec une version nightly figée, sous Linux.
- **Bancs.** Les micro-bancs `criterion` vivent dans le `benches/` de leur crate. `bench/` contient les scripts. Un workflow nocturne pousse l'historique sur la branche `bench-data`.
- **Documentation.** Elle reste à la racine (aucun lien existant ne change). `xtask book` génère les sources mdBook, `xtask links` vérifie chaque lien relatif et chaque ancre de tous les `.md` versionnés, et `docs.yml` construit le site puis le déploie sur GitHub Pages.

**Pile :** `cargo-fuzz` 0.13.2 et `libfuzzer-sys` 0.4.13 (nightly figée), `criterion` 0.8.2, `mdbook` 0.5.4, actions GitHub pour Pages et pour l'historique des bancs.

**Spécification :**
- [CAHIER_DES_TACHES.md](../../../CAHIER_DES_TACHES.md) : T01.14, T01.15, T01.17 ;
- [CAHIER_DES_CHARGES.md](../../../CAHIER_DES_CHARGES.md) : ENF-SEC-03, ENF-PERF, ENF-DOC-01 ;
- [STRATEGIE_DE_TESTS.md](../../../STRATEGIE_DE_TESTS.md) : §5, §9.1 à §9.3, §13.2, §13.4 ;
- [CLAUDE.md](../../../CLAUDE.md) : §4.12 (versions figées), §5, §10, §11.

## Contraintes globales

- Toute version d'outil est figée (CLAUDE.md §4.12) :
  - `cargo-fuzz` et `mdbook` dans `scripts/tools.versions` ;
  - la version nightly du fuzzing dans `fuzz/rust-toolchain.toml` ;
  - les actions épinglées par empreinte de commit :
    - `benchmark-action/github-action-benchmark` `4322e5726e6334590d251fc4f92bec0efafc45dc` (v1.22.2) ;
    - `actions/configure-pages` `45bfe0192ca1faeb007ade9deae92b16b8254a0d` (v6.0.0) ;
    - `actions/upload-pages-artifact` `fc324d3547104276b827a68afc52ff2a11cc49c9` (v5.0.0) ;
    - `actions/deploy-pages` `368f82528645a54fb793d4d04e342629a3f51346` (v5.0.1) ;
    - `actions/cache` `55cc8345863c7cc4c66a329aec7e433d2d1c52a9` (v6.1.0).
- Permissions des jetons : `contents: read` par défaut. Elles ne sont élargies que dans le seul travail qui en a besoin : `issues: write` pour le fuzzing nocturne, `contents: write` pour les bancs, `pages: write` et `id-token: write` pour le déploiement.
- Durée de fuzzing nocturne : **1 h par cible** (STRATEGIE §13.2).
- Aucun emoji ; code et scripts en ASCII (CLAUDE.md §4.7).
- Test d'abord ; mutation quand un test passe du premier coup.

## Décisions à valider avec le plan

1. **Fuzzing sous Linux seulement, avec une nightly figée.** `cargo fuzz` exige une version nightly (`-Zsanitizer`) et ne fonctionne bien que sous Linux et macOS. Le fichier `fuzz/rust-toolchain.toml` épingle `nightly-2026-10-01` (vérifiée disponible). La version stable 1.99.0 du dépôt ne change pas. En contrepartie, le corpus est rejoué en stable à chaque demande de fusion, sur les trois systèmes, par un test de `ndaal-fuzz`. Localement, `scripts/fuzz.sh` passe par Docker sous Windows (§4.8).
2. **Cible d'exemple `common_codec`** : les décodeurs existants de `ndaal-common` (varint, zigzag, octets préfixés, chaînes, entiers, identifiants base32 Crockford). Elle vérifie qu'aucune entrée ne provoque de panique et que tout ce qui est décodé se réencode à l'identique. Ce sont les premiers décodeurs du projet ; les cibles de STRATEGIE §5 suivront avec leurs décodeurs.
3. **Corpus.** Le corpus de départ est versionné dans `fuzz/corpus/<cible>/`. Le corpus qui grandit chaque nuit est conservé dans le cache de GitHub Actions et publié en artefact ; aucun robot ne pousse sur `main`. On le réintègre à la main, par une demande de fusion (`cargo fuzz cmin`).
4. **Un plantage ouvre un ticket.** Le travail nocturne crée un ticket « fuzz: crash in `<cible>` » (étiquette `fuzz`), ou commente le ticket encore ouvert pour la même cible. L'entrée fautive est publiée en artefact. La demande de fusion lance aussi un essai de 60 s par cible si `fuzz/` ou `ndaal-fuzz` change.
5. **Bancs sur les machines partagées de GitHub, en attendant D-05.** Les mesures y sont bruitées (STRATEGIE §9.1) : elles servent à tracer une tendance, sans alerte ni ticket de régression. L'historique est poussé sur une branche dédiée, `bench-data`, par `github-action-benchmark` ; `main` n'est pas touchée. Point reporté P-42 : passer sur des machines dédiées, avec alerte à 5 %, après D-05.
6. **Le site garde les documents à la racine.** `xtask book` copie les documents dans `docs/book/src/` (ignoré par git), écrit le `SUMMARY.md` et réécrit les liens vers des fichiers hors du site (par exemple `deploy/docker/Dockerfile`) en liens vers GitHub. Le déploiement sur GitHub Pages se fait à chaque fusion sur `main` et chaque nuit (pour reprendre le graphe des bancs, publié sous `/bench/`). **Vous** devez activer Pages : *Settings → Pages → Source : GitHub Actions*.
7. **Vérificateur de liens maison (`xtask links`).** `mdbook-linkcheck` n'est plus maintenu et ne suit pas mdBook 0.5. Le vérificateur contrôle tous les `.md` versionnés : fichier cible existant, et ancre existante, calculée comme GitHub (et mdBook) le font, y compris pour les titres en double. Il est ajouté à CLAUDE.md §11 (`./scripts/check-links.sh`) et à la tâche `checks` de la CI. Les liens cassés trouvés dans la documentation actuelle sont corrigés dans ce lot.
8. **Diagrammes Mermaid affichés comme du code dans le site.** Les afficher en dessins demanderait d'embarquer `mermaid.js` (environ 3 Mo) ou un préprocesseur de plus. GitHub, lui, les dessine. Point reporté P-43.

## Points à surveiller en revue

1. **Titres avec accents, accents graves de code, tirets cadratins ou en double** (« ADR-032 — Frontal HTTP sur `axum` », deux « Contexte ») : l'ancre calculée est celle de GitHub (`adr-032--frontal-http-sur-axum`, `contexte-1`). Testé à la tâche 3.
2. **Lien vers un fichier hors du site** (`deploy/docker/Dockerfile`, `.github/workflows/ci.yml`) : dans le site, il pointe vers GitHub, jamais vers une page 404. Testé à la tâche 3.
3. **Fichier du corpus qui fait paniquer la cible** : le test de rejeu échoue en nommant le fichier. Testé à la tâche 1.
4. **Plantage nocturne alors qu'un ticket est déjà ouvert pour la cible** : un commentaire est ajouté, pas un second ticket. Vérifié à la tâche 1 par `check-workflows.sh` et par relecture du script, faute de pouvoir provoquer un plantage nocturne avant la fusion.
5. **Première exécution des bancs, branche `bench-data` absente** : la branche est créée. Vérifié à la tâche 2 au premier lancement manuel après la fusion.

---

### Tâche 1 : fuzzing (T01.14)

**Fichiers :**
- Créer :
  - `crates/ndaal-fuzz/{Cargo.toml, src/lib.rs, tests/corpus.rs}` ;
  - `fuzz/{Cargo.toml, rust-toolchain.toml, fuzz_targets/common_codec.rs}` ;
  - `fuzz/corpus/common_codec/*`, des graines écrites à la main, chacune commentée dans `fuzz/corpus/README.md` ;
  - `scripts/fuzz.sh` ;
  - `.github/workflows/fuzz.yml`.
- Modifier :
  - `scripts/tools.versions` (`cargo-fuzz 0.13.2`) ;
  - `Cargo.toml` (`exclude = ["fuzz"]`) ;
  - `.gitignore` (`fuzz/target/`, `fuzz/artifacts/`).

**Interfaces (produit) :**
- `ndaal_fuzz::common_codec(data: &[u8])` : elle panique si une propriété est violée, c'est son rôle.
  - Pour chaque décodeur, elle décode depuis `data`.
  - Si le décodage réussit, elle réencode avec le `put_*` correspondant et vérifie qu'on retrouve les mêmes octets consommés. Exception : la représentation canonique du varint, où l'on vérifie que la valeur se redécode à l'identique.
  - `decode_crockford` d'une chaîne valide doit faire l'aller-retour avec l'encodeur.
- `ndaal_fuzz::TARGETS: &[(&str, fn(&[u8]))]`, la table cible → fonction, partagée par le test de rejeu.
- `scripts/fuzz.sh <cible> [secondes]` lance `cargo +<nightly de fuzz/rust-toolchain.toml> fuzz run <cible> fuzz/corpus/<cible> -- -max_total_time=<secondes>`, ou passe par le conteneur Rust figé si l'hôte est Windows.

- [ ] **Tests d'abord** (`crates/ndaal-fuzz/tests/corpus.rs`) :
  - `every_target_has_a_corpus` : chaque nom de `TARGETS` a un dossier non vide sous `fuzz/corpus/`, et chaque dossier correspond à une cible. Il est lu par `ndaal_io::real::RealDisk`.
  - `corpus_replays_without_panic` : chaque fichier passe dans sa cible, sous `catch_unwind` (permis dans les tests seulement) pour que le message d'échec nomme le fichier.
  - `common_codec_rejects_without_panicking` : une table d'entrées limites (vide, varint de 11 octets, longueur préfixée plus grande que le reste, UTF-8 invalide, base32 avec `U`).
  - Mutation : remplacer une vérification d'aller-retour par une vérification inversée doit faire échouer le rejeu.
- [ ] **`fuzz.yml`** :
  - déclencheurs : `schedule` (chaque nuit à 02:00 UTC), `workflow_dispatch`, et `pull_request` filtré sur `fuzz/**` et `crates/ndaal-fuzz/**` ;
  - la liste des cibles est donnée par `cargo fuzz list` ;
  - durée : 3600 s par cible la nuit, 60 s sur une demande de fusion ;
  - le corpus est restauré et sauvé par `actions/cache` (clé `fuzz-corpus-<cible>-<run_id>`, restauration par préfixe) ;
  - sur échec la nuit : artefact `fuzz/artifacts/`, puis `gh issue list --label fuzz --search "<cible>"` suivi de `gh issue comment` ou de `gh issue create --label fuzz`.
- [ ] Vérification : `check-workflows.sh`, et `scripts/fuzz.sh common_codec 60` lancé via Docker si le démon répond. Sinon, c'est l'essai de 60 s de la demande de fusion qui fait foi.

---

### Tâche 2 : bancs (T01.15)

**Fichiers :**
- Créer :
  - `crates/ndaal-common/benches/codec.rs` ;
  - `bench/README.md` ;
  - `bench/micro.sh` ;
  - `.github/workflows/bench.yml`.
- Modifier :
  - `Cargo.toml` (`criterion = { version = "0.8.2", default-features = false, features = ["cargo_bench_support"] }`) ;
  - `crates/ndaal-common/Cargo.toml` (`[dev-dependencies] criterion`, `[[bench]] name = "codec" harness = false`).

**Interfaces :**
- Bancs `varint/encode`, `varint/decode` et `crockford/decode` : 1024 valeurs fixes, générées par une suite déterministe, sans `rand`.
- `bench/micro.sh` lance `cargo bench --workspace -- --output-format bencher | tee bench-output.txt`. Ce format est celui que lit `github-action-benchmark`, avec `tool: cargo`.
- `bench.yml` : déclenché chaque nuit à 03:00 UTC et par `workflow_dispatch`, sur `ubuntu-24.04`. Il passe à l'action `github-action-benchmark` :
  - `gh-pages-branch: bench-data` ;
  - `benchmark-data-dir-path: micro` ;
  - `auto-push: true` ;
  - `comment-on-alert: false` et `fail-on-alert: false` (décision 5).

- [ ] **Vérification** :
  - `cargo bench -p ndaal-common -- --test` lance chaque banc une fois, comme un test ; il est ajouté à la CI `test` sous Linux pour que les bancs ne pourrissent pas ;
  - `cargo clippy --all-targets` couvre `benches/` ;
  - `check-workflows.sh`.

---

### Tâche 3 : site de documentation et vérification des liens (T01.17)

**Fichiers :**
- Créer :
  - `xtask/src/docs/{mod.rs, slug.rs, links.rs, book.rs}` ;
  - `docs/book.toml` ;
  - `scripts/check-links.sh` ;
  - `scripts/build-docs.sh` ;
  - `.github/workflows/docs.yml`.
- Modifier :
  - `xtask/src/main.rs` (sous-commandes `links` et `book`) ;
  - `scripts/tools.versions` (`mdbook 0.5.4`) ;
  - `.gitignore` (`docs/book/`) ;
  - `.github/workflows/ci.yml` (étape « Links » dans `checks`) ;
  - `CLAUDE.md` §11 ;
  - les documents où des liens sont cassés.

**Interfaces :**
- `slug::anchors(markdown: &str) -> BTreeSet<String>` :
  - parcourt les titres ATX, hors blocs de code délimités ;
  - suit l'algorithme de GitHub : minuscules, suppression de tout ce qui n'est ni alphanumérique Unicode, ni espace, ni `-`, ni `_`, puis chaque espace devient `-` ;
  - un titre en double reçoit le suffixe `-1`, `-2`, et ainsi de suite.

  Les ancres HTML explicites (`<a id="...">`) sont aussi reconnues.
- `links::check(files: &[(String, String)]) -> Vec<BrokenLink { file, line, target, reason }>` :
  - vérifie les liens Markdown relatifs, hors blocs de code, hors `http(s):` et hors `mailto:` ;
  - accepte une cible qui est un fichier ou un dossier versionné ;
  - vérifie l'ancre si la cible est un `.md`.

  Les fichiers sont lus par `git ls-files '*.md'`, comme le fait déjà l'hygiène.
- `book::generate(out: &Path)` :
  - copie la liste fixe des documents (`BOOK_PAGES`, avec leur titre de chapitre et leur ordre) ;
  - écrit `SUMMARY.md` ;
  - réécrit chaque lien relatif vers un fichier qui n'est pas une page du livre en `https://github.com/LordPhenixDeNetra/Ndaal/blob/main/<chemin>`.

  `BOOK_PAGES` : `README.md` (introduction), CAHIER_DES_CHARGES, ARCHITECTURE, DECISIONS, FORMATS_DE_DONNEES, API_ET_PROTOCOLES, EXPLOITATION, STRATEGIE_DE_TESTS, CAHIER_DES_TACHES, POINTS_REPORTES, GLOSSAIRE, CONTRIBUTING, CLAUDE (« Règles de code »), SECURITY, CODE_OF_CONDUCT.
- `docs.yml` :
  - **demande de fusion** : liens, `scripts/build-docs.sh` (`cargo run -p xtask -- book` puis `mdbook build docs`) ;
  - **`main` (fusion) et chaque nuit à 05:00 UTC** : construction du site, ajout du contenu de la branche `bench-data` (s'il existe) sous `docs/book/html/bench/`, puis `upload-pages-artifact` et `deploy-pages`.

- [ ] **Tests d'abord** (unitaires, dans `xtask`) :
  - `slug_matches_github` : la table doit contenir au moins `("ADR-032 — Frontal HTTP sur `axum`", "adr-032--frontal-http-sur-axum")`, `("14.3 HTTP et gRPC", "143-http-et-grpc")`, `("Référence de configuration", "référence-de-configuration")` et `("C++ / Rust!", "c--rust")` ;
  - `duplicate_headings_get_suffixes` ;
  - `headings_inside_code_blocks_are_ignored` ;
  - `broken_file_and_anchor_are_reported` ;
  - `external_and_code_block_links_are_skipped` ;
  - `links_outside_the_book_point_to_github` ;
  - `summary_lists_every_page_in_order`.
- [ ] **Vérifications** :
  - `scripts/check-links.sh` doit passer sur le dépôt après correction des liens cassés ; leur liste va dans le registre ;
  - `scripts/build-docs.sh` doit construire le site, si `mdbook` est installé (`install-tools.sh mdbook` : demande d'accord, car c'est une installation sur la machine, CLAUDE.md §13). Sinon, c'est la CI qui fait foi.

---

### Tâche 4 : documentation, suivi, vérifications

**Fichiers :** `STRATEGIE_DE_TESTS.md` (§5 et §13.4 : `fuzz.yml`, `bench.yml`, `docs.yml` tels que réalisés), `CAHIER_DES_TACHES.md`, `POINTS_REPORTES.md`, `CONTRIBUTING.md` (lancer le fuzzing, ajouter une cible, voir le site en local).

- [ ] **Points reportés** :
  - P-42 : bancs sur machines dédiées et alerte à 5 %, après D-05 ;
  - P-43 : rendu des diagrammes Mermaid dans le site ;
  - P-44 : les cibles de fuzzing de STRATEGIE §5, à ajouter avec leurs décodeurs.
- [ ] **Chaîne complète de CLAUDE.md §11**, plus `check-links.sh`.
- [ ] **Revue finale** par un relecteur neuf.
- [ ] **Compte rendu**, avec les commandes git et le contenu du formulaire de demande de fusion.
- [ ] **Cochage des tâches** :
  - T01.17 est cochée si la CI construit le site et vérifie les liens ;
  - T01.14 et T01.15 ne le sont qu'après une première nuit réussie sur `main`, constatée après la fusion.
