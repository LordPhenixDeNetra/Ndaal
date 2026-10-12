# CLAUDE.md — Règles de travail sur Ndaal

Ndaal est un moteur de données en Rust qui remplace Redis/Valkey, Qdrant et MinIO par un seul système (clé-valeur, vecteurs, objets) avec des transactions atomiques entre les trois. Ce fichier fixe les règles de code. Il s'applique à toute personne et à tout agent qui modifie le dépôt.

Les préférences personnelles d'un contributeur (langue des réponses, façon de committer) vont dans un `CLAUDE.local.md`, exclu de git, jamais ici.

## 1. Sources de vérité

Avant de coder, lire ce qui concerne la tâche. En cas de conflit, l'ordre de priorité est :

1. [CAHIER_DES_CHARGES.md](CAHIER_DES_CHARGES.md) — **ce que** l'on construit (exigences `EF-*`, `ENF-*`).
2. [DECISIONS.md](DECISIONS.md) — décisions déjà prises (`ADR-*`). Ne pas les rediscuter dans le code ; proposer un nouvel ADR.
3. [ARCHITECTURE.md](ARCHITECTURE.md), [FORMATS_DE_DONNEES.md](FORMATS_DE_DONNEES.md), [API_ET_PROTOCOLES.md](API_ET_PROTOCOLES.md) — **comment**.
4. [CAHIER_DES_TACHES.md](CAHIER_DES_TACHES.md) — tâche en cours (`Tnn.mm`), dépendances et critère de fin.
5. [STRATEGIE_DE_TESTS.md](STRATEGIE_DE_TESTS.md) — comment prouver que c'est fini.

Si le code doit s'écarter de la documentation, **mettre à jour la documentation dans la même modification**. Une documentation fausse est pire qu'une absence de documentation.

## 2. Principes non négociables

| Règle | Pourquoi |
|---|---|
| Aucune écriture acquittée ne peut être perdue (ENF-DUR) | C'est une base de données : la perte de données est le pire défaut possible |
| Jamais plus lent que Valkey, Qdrant ou MinIO à durabilité égale (ENF-COMP, ADR-027) | Sinon Ndaal n'a aucune raison d'exister |
| Un cœur possède ses données : pas de verrou partagé sur le chemin des écritures (ADR-003) | Latence prévisible, passage à l'échelle par cœur |
| Toute file est bornée ; en surcharge on répond « occupé » (P8) | Mémoire bornée, pas d'effondrement |
| Toute donnée externe (réseau, disque, configuration) est validée ; jamais de `panic!` dessus | Sécurité et robustesse |
| Tout format sur disque et tout message réseau est versionné (ENF-QUAL-03) | Mises à jour sans perte et retour arrière possible |

## 3. Architecture du code

### 3.1 Workspace et dépendances entre crates

- Le dépôt est un workspace Cargo ; l'organisation est décrite dans [ARCHITECTURE §3](ARCHITECTURE.md#3-organisation-du-code).
- **Les dépendances vont vers le bas** selon le graphe de l'architecture. Une crate basse (`ndaal-common`, `ndaal-io`) ne dépend jamais d'une crate haute. Aucun cycle.
- Une crate = une responsabilité. Si une crate a besoin d'un type d'une crate sœur, le type descend dans `ndaal-common` ou dans une interface (trait) au niveau inférieur.
- Les versions des dépendances externes sont **centralisées** dans le `Cargo.toml` racine (`[workspace.dependencies]`).
- Les frontaux de protocole (`ndaal-proto-*`) ne contiennent **aucune logique métier** : ils décodent, autorisent, appellent le cœur, encodent.

### 3.2 Effets de bord

- **Aucune crate ne lit l'horloge, le disque, le réseau ou l'aléa directement.** Tout passe par les traits de `ndaal-io` (ADR-016, ADR-017). Un lint maison l'impose en CI.
- C'est ce qui rend la simulation déterministe possible : un code qui contourne `ndaal-io` est un code qu'on ne peut pas tester sous pannes.

### 3.3 Interfaces publiques

- Visibilité minimale : `pub(crate)` par défaut, `pub` seulement pour ce que les autres crates utilisent réellement.
- Une API publique est un contrat : un changement incompatible exige une mise à jour de [API_ET_PROTOCOLES.md](API_ET_PROTOCOLES.md) et, pour les formats, de [FORMATS_DE_DONNEES.md](FORMATS_DE_DONNEES.md) (§14, politique de compatibilité).
- Préférer des types qui rendent les états invalides impossibles (*newtypes* pour les identifiants : `PartitionId`, `Lsn`, `NamespaceId` ; énumérations plutôt que booléens).

## 4. Règles Rust

### 4.1 Style et outillage

- `cargo fmt` et `cargo clippy --all-targets -- -D warnings` doivent passer (ENF-QUAL-02). Ne jamais désactiver un lint sans un commentaire qui explique pourquoi.
- MSRV déclarée dans `Cargo.toml` et vérifiée en CI (ENF-PORT-04). Ne pas utiliser de fonctionnalité plus récente sans relever la MSRV explicitement.
- Nommage : code, identifiants et messages de journalisation en **anglais** ; documentation de conception en **français**.

### 4.2 Erreurs

- Erreurs **typées par crate** avec `thiserror`. Pas de `anyhow` dans les bibliothèques (accepté dans `ndaal-cli` et les tests).
- Conversion vers les codes de protocole (`ERR …` RESP, codes S3, codes HTTP) **uniquement dans les frontaux** ([API §10](API_ET_PROTOCOLES.md#10-codes-derreur)).
- `unwrap()` et `expect()` interdits hors tests, sauf pour un invariant prouvé localement, avec `expect("raison de l'invariant")`.
- Une erreur d'E/S n'est jamais avalée : elle remonte, ou elle est journalisée **et** comptée dans une métrique.
- La cause est toujours conservée (`#[source]` / `#[from]`) : on ajoute du contexte, on ne remplace jamais l'erreur d'origine.
- Une erreur est traitée **une seule fois** : remontée, ou journalisée, jamais les deux à chaque niveau.
- Messages d'erreur en anglais, en minuscules, sans ponctuation finale (convention Rust) ; ils disent ce qui a échoué et avec quelle valeur, sans secret.
- L'identifiant de requête accompagne toute erreur renvoyée au client et tout journal associé ([ARCHITECTURE §18.1](ARCHITECTURE.md#181-règles-de-propagation-des-erreurs)).
- Une panique est un bogue : les binaires utilisent `panic = "abort"` et le nœud repart du journal (ADR-030). `catch_unwind` est interdit pour masquer une panique.

### 4.3 `unsafe`

- `#![forbid(unsafe_code)]` partout, sauf `ndaal-io`, `ndaal-runtime`, `ndaal-vector` (SIMD) et `ndaal-wal` (tampons alignés).
- Chaque bloc `unsafe` porte un commentaire `// SAFETY:` qui justifie chaque invariant, est encapsulé derrière une API sûre, et est couvert par Miri ou un test dédié (ENF-SEC-01).

### 4.4 Asynchrone et concurrence

- Modèle thread-per-core : `tokio` en mode `current_thread` par cœur (ADR-003). Ne pas introduire de runtime multi-thread ni de `Mutex` partagé sur le chemin critique.
- Les communications entre cœurs passent par la messagerie de `ndaal-runtime`, pas par de la mémoire partagée.
- Aucune opération bloquante (E/S synchrone, calcul long) dans une tâche asynchrone d'un cœur `shard` ; le calcul lourd va sur les cœurs dédiés (P4).
- Toute attente a un délai ; toute file a une borne.

### 4.5 Performance

- Le chemin court (`GET`/`SET`, recherche, petits objets) est **sans allocation inutile et sans copie** : `bytes::Bytes`, emprunts, tampons réutilisés.
- Pas d'optimisation au jugé : mesurer d'abord (`criterion`, `perf`, graphes de flammes), puis optimiser, puis remesurer.
- Toute modification du chemin critique est mesurée avec le banc concerné **et** son banc comparatif `cmp-*` ([STRATEGIE §9.4](STRATEGIE_DE_TESTS.md#94-bancs-comparatifs-parité-avec-les-services-remplacés)). Une régression > 5 % est un défaut.

### 4.6 Taille et lisibilité

Seuils vérifiés en CI. Les limites « dure » bloquent la fusion ; les limites « cible » déclenchent un découpage dès que possible.

| Élément | Cible | Limite dure | Outil |
|---|---|---|---|
| Fichier source `.rs` (hors tests et code généré) | ≤ 400 lignes | 800 lignes | Script CI (`tokei`) |
| Fonction | ≤ 50 lignes | 100 lignes | `clippy::too_many_lines` (`too-many-lines-threshold = 100` dans `clippy.toml`) |
| Paramètres d'une fonction | ≤ 5 | 7 | `clippy::too_many_arguments` (`too-many-arguments-threshold = 7`) |
| Imbrication (`if`/`match`/boucles) | ≤ 3 niveaux | 4 niveaux | Revue ; `clippy::cognitive_complexity` |
| Largeur de ligne | 100 colonnes | 100 colonnes | `rustfmt.toml` : `max_width = 100` |
| Module (dossier) | ≤ 10 fichiers | — | Revue |

- Dépasser une cible = découper : extraire un sous-module, un type, une fonction nommée. Ne pas contourner la limite en compactant le code.
- Exceptions autorisées, à déclarer explicitement : code **généré** (table des commandes RESP, Protocol Buffers) dans un fichier `*_generated.rs` ou sous `generated/` ; **tables de données** (vecteurs de test, constantes) ; noyaux **SIMD** dont le déroulage est voulu. Un `#[allow(clippy::too_many_lines)]` porte toujours un commentaire qui justifie l'exception.
- Fichiers de tests : limite dure portée à 1 500 lignes ; au-delà, découper par fonctionnalité.
- Pas de nombres magiques : toute constante non évidente est nommée (`const MAX_FRAME_LEN: usize = …`) avec un commentaire d'unité et d'origine.
- Pas de code mort ni de code commenté : on le supprime, Git s'en souvient. `#[allow(dead_code)]` interdit hors tests.
- Commentaires : ils expliquent le **pourquoi** (invariant, contrainte, renvoi à un ADR ou une exigence), pas le **quoi**. Chaque élément `pub` a sa documentation `///` (ENF-DOC-02).

### 4.7 Caractères et emojis

- **Aucun emoji**, nulle part dans le dépôt : code, commentaires, chaînes de caractères, messages d'erreur, journaux, métriques, noms de fichiers, messages de commit, demandes de fusion, documentation.
- Identifiants (variables, fonctions, types, modules, crates, fichiers) en **ASCII uniquement**.
- Dans le code source `.rs`, commentaires et chaînes également en ASCII, sauf les données de test qui vérifient précisément le traitement de l'Unicode : elles sont alors écrites avec des échappements (`"\u{00e9}"`), jamais en caractères littéraux invisibles ou ambigus.
- La documentation Markdown peut utiliser le français accentué et les caractères typographiques (« », —, →, tracés de schémas), mais pas d'emoji.
- Encodage UTF-8 sans BOM, fins de ligne LF (`.gitattributes` : `* text=auto eol=lf`), pas d'espace en fin de ligne, une ligne vide en fin de fichier.
- Vérifié en CI par un script qui rejette tout emoji (plages Unicode `U+1F000–U+1FAFF`, `U+2600–U+27BF`, sélecteur `U+FE0F`) et tout caractère non ASCII dans les fichiers `.rs`, `.toml`, `.yml`.

### 4.8 Portabilité (Linux, macOS, Windows)

- Linux est la plateforme de production ; macOS et Windows sont des plateformes de développement **de premier rang** : la suite de tests complète doit y passer (ENF-PORT-02).
- Le code propre à un système est confiné à `ndaal-io` et `ndaal-runtime`, derrière `#[cfg(target_os = "...")]`, toujours avec un repli. Aucun `#[cfg]` de système ailleurs.
- Chemins manipulés avec `std::path::Path`/`PathBuf`, jamais par concaténation de chaînes ni avec un séparateur `/` écrit en dur.
- Pas d'hypothèse sur le système de fichiers : sensibilité à la casse, longueur des chemins sous Windows, renommage atomique, permissions Unix (`#[cfg(unix)]` si nécessaire).
- Les scripts du dépôt sont appelables depuis la CI sur les trois systèmes ; tout ce qui tourne en CI doit pouvoir se lancer en local, au besoin via `docker compose` (`deploy/docker/`).

### 4.9 Internationalisation

- Seuls la console web, les messages de la CLI et la documentation utilisateur sont traduits (ADR-029). **Jamais** les erreurs de protocole, les journaux, les métriques, les noms de configuration ni la sortie JSON.
- Aucun texte destiné à un humain écrit en dur dans la CLI ou la console : une clé Fluent dans `locales/en/*.ftl`, traduite dans `locales/fr/*.ftl`. La CI vérifie que chaque clé existe dans chaque langue.
- Les clés sont en anglais, en minuscules avec des tirets, préfixées par le domaine : `cli-backup-done`, `console-ns-empty`.
- Pas de phrase construite par concaténation : utiliser les variables et les pluriels de Fluent (`{ $count -> [one] ... *[other] ... }`).
- Les fichiers `.ftl` peuvent contenir des accents (§4.7 ne vise que le code), mais pas d'emoji.

### 4.10 Console web (`console/`)

- Svelte + TypeScript en mode **strict** (`strict: true`, pas de `any` implicite) ; `svelte-check` et le lint sans avertissement (ADR-031).
- La console n'appelle **que l'API HTTP publique**. Une fonction qui manque dans l'API s'ajoute d'abord à l'API (documentée dans [API_ET_PROTOCOLES.md](API_ET_PROTOCOLES.md)), puis à la console. Masquer un bouton n'est jamais un contrôle d'accès : c'est l'API qui refuse.
- Aucun texte affiché écrit en dur : tout passe par les clés Fluent partagées avec la CLI (§4.9).
- Listes toujours paginées ou virtualisées ; jamais de chargement complet d'une collection, d'un compartiment ou d'un espace de clés (ENF-UI-03).
- Pas de `innerHTML` ni de `{@html}` sur une donnée venant du serveur : les valeurs des clés, charges utiles et métadonnées sont des données non fiables.
- Accessibilité dès l'écriture : éléments sémantiques, libellés, focus visible, navigation au clavier (ENF-UI-01).
- Dépendance npm nouvelle : même exigence que pour Cargo (§4.11), versions verrouillées dans `package-lock.json`.
- Mêmes règles de taille (§4.6 : fichiers et composants ≤ 400 lignes visés, 800 au maximum) et d'emojis (§4.7) que pour le code Rust.

### 4.11 Dépendances

- Avant d'ajouter une dépendance : la bibliothèque standard ou une dépendance déjà présente suffit-elle ?
- Licences autorisées et interdites : contrainte C-02, vérifiée par `cargo deny`. Pas de code C/C++ dans le chemin critique (C-01).
- Une nouvelle dépendance structurante (runtime, sérialisation, cryptographie, stockage) exige un ADR.

### 4.12 Versions figées des outils

Une CI qui casse parce qu'un outil s'est mis à jour tout seul fait perdre du temps et masque les vraies régressions. Toute version d'outil est donc **figée** et changée volontairement, par une demande de fusion dédiée.

| Outil | Où la version est figée |
|---|---|
| Rust (compilateur, `clippy`, `rustfmt`) | `rust-toolchain.toml` : version exacte (`channel = "1.xx.y"`), jamais `stable` |
| Outils Cargo (`cargo-deny`, `cargo-nextest`, `cargo-fuzz`, `cargo-llvm-cov`) | `scripts/tools.versions`, installés avec `cargo install --locked --version …` en local comme en CI |
| Node.js et npm (console) | `console/.nvmrc` et champ `engines` de `package.json` ; dépendances verrouillées par `package-lock.json`, installées avec `npm ci` |
| Playwright et ses navigateurs | Version exacte dans `package.json` |
| Images Docker de base et de référence (Valkey, Qdrant, MinIO) | Épinglées par empreinte (`@sha256:…`), pas seulement par étiquette |
| Actions GitHub | Épinglées par empreinte de commit (§10) |

Les mises à jour proposées par Dependabot sont relues comme du code : notes de version lues, CI complète verte, bancs comparatifs vérifiés pour Rust et les références.

## 5. Tests

- **Test d'abord** : pour toute nouvelle logique et toute correction, écrire le test, le **voir échouer**, puis écrire le code (TDD). Un test qui n'a jamais échoué ne prouve rien.
- **Un bogue = un test** qui échouait avant la correction (ENF-QUAL-04), y compris la graine de simulation qui l'a révélé.
- Structures de données critiques : tests de **modèle** (comparaison avec un `BTreeMap` ou un `Vec` naïf) via `proptest`.
- Tout ce qui décode une entrée externe ou un format a une cible de **fuzzing** (`cargo fuzz`).
- Tout composant à effets de bord a ses scénarios de **simulation déterministe** (`ndaal-sim`), avec pannes.
- Compatibilité : jugée avec les **vrais clients** (`redis-py`, `boto3`, `aws-cli`…), pas seulement avec les nôtres.
- Couverture : ≥ 85 % pour les crates du moteur de stockage, ≥ 70 % au total (ENF-QUAL-01). C'est un indicateur, pas un objectif : un test sans assertion utile ne compte pas.
- Ne jamais désactiver, ignorer (`#[ignore]`) ou affaiblir un test pour faire passer la CI. Corriger le code, ou documenter le problème dans un ticket et dans le registre du cahier des tâches.

## 6. Observabilité

- Journaux structurés avec `tracing` ; jamais de `println!` dans le code serveur.
- Chaque nouveau chemin significatif expose ses métriques `ndaal_*` (compteurs, histogrammes de latence) ; les noms suivent [EXPLOITATION §8](EXPLOITATION.md#8-supervision-et-alertes).
- Jamais de secret, de clé ni de donnée utilisateur dans les journaux ou les métriques.

## 7. Sécurité

- Configuration sûre par défaut : écoute sur `127.0.0.1`, refus d'exposer une interface publique sans authentification (ENF-SEC-04).
- Limites explicites sur toutes les entrées (taille, profondeur, nombre d'éléments) : voir [API §11](API_ET_PROTOCOLES.md#11-limites-par-défaut).
- Comparaisons de secrets en temps constant ; cryptographie via `ring` ou `rustls` uniquement (ADR-020).

## 8. Évolution du code

- **Petites modifications** : une demande de fusion = une intention, relisible en moins de 30 minutes. Découper sinon.
- **Pas d'abstraction spéculative** : pas de trait avec une seule implémentation, pas de configuration pour une valeur qui ne change jamais. Exception prévue : les traits de `ndaal-io`, qui ont deux implémentations (réelle et simulée).
- Trois répétitions avant de factoriser : on duplique une fois, on factorise à la troisième.
- Un raccourci délibéré (algorithme naïf, verrou global, limite fixe) est marqué d'un commentaire `// TODO(Tnn.mm):` qui décrit la limite et renvoie à la tâche qui la lèvera.
- Changement de format ou de protocole : nouvelle version du format, lecture des anciennes versions conservée, corpus de compatibilité mis à jour (ENF-COMPAT-04).
- Fonctionnalité risquée : derrière une option de configuration désactivée par défaut jusqu'à validation.

## 9. Suivi du travail

- Chaque modification se rattache à une tâche `Tnn.mm` et aux exigences qu'elle couvre.
- Une tâche n'est cochée dans [CAHIER_DES_TACHES.md](CAHIER_DES_TACHES.md) que si son critère de fin est vérifié, ses tests écrits et la CI verte ; mettre à jour le tableau de bord.
- Une tâche découverte en cours de route est ajoutée avec le numéro suivant libre et consignée dans le registre des modifications ; on ne renumérote ni ne supprime jamais une tâche.
- Une décision d'architecture nouvelle ou modifiée devient un ADR dans [DECISIONS.md](DECISIONS.md).
- **Tout point reporté** (remarque mineure de revue non traitée, décision remise à plus tard, limite connue acceptée provisoirement) va dans [POINTS_REPORTES.md](POINTS_REPORTES.md), jamais seulement dans un registre de travail non versionné ou dans un message.

## 10. Git

- Messages de commit au format *Conventional Commits*, avec la portée de la crate : `feat(kv): expiration active par roue temporelle (T03.07)`.
- Signature DCO (`Signed-off-by`) obligatoire.
- Jamais de commit direct sur la branche principale ; jamais de `--no-verify`.
- La CI GitHub Actions fait foi ([STRATEGIE §13.4](STRATEGIE_DE_TESTS.md#134-mise-en-œuvre-sur-github-actions)) : une modification n'est fusionnable que si tous les workflows requis sont verts. Toute action tierce ajoutée à un workflow est épinglée par empreinte de commit.

## 11. Avant de déclarer une tâche terminée

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo nextest run
cargo deny check
./scripts/check-hygiene.sh   # taille des fichiers, emojis, caractères non ASCII, fins de ligne
./scripts/check-linux.sh     # clippy et tests compilés pour Linux, depuis macOS ou Windows
```

**Sous Windows ou macOS, `check-linux.sh` est obligatoire.** Le code propre à Linux (`io_uring`, `O_DIRECT`, épinglage des threads) est derrière `#[cfg(target_os = "linux")]` : un `cargo clippy` local ne le compile même pas, et ses erreurs n'apparaîtraient qu'en CI. Le script lance `clippy` et `nextest` dans le conteneur Rust de la version figée (Docker) ; sous Windows, WSL2 convient aussi.

Pour la console : `npm run check`, `npm run lint`, `npm test` et `npx playwright test` dans `console/`.

Puis, selon ce qui a été touché : simulation (`ndaal-sim`), fuzzing de la cible concernée, banc de performance et banc comparatif `cmp-*`. Indiquer dans la demande de fusion ce qui a été exécuté et le résultat, y compris ce qui a échoué ou n'a pas pu être lancé.

## 12. Méthode de travail

1. **Travail d'envergure** (nouvelle crate, nouveau protocole, changement de format, tâche L ou XL) : exploration des options, puis **spécification**, puis **plan** découpé en étapes vérifiables, avec validation du responsable du projet à chaque étape, avant d'écrire le code.
2. **Exécution** : petites étapes, test d'abord (§5), CI locale verte (§11) à chaque étape.
3. **Revue finale** par un relecteur qui n'a pas écrit le code (une personne ou un agent sans le contexte de l'auteur). Les points **critiques** et **importants** sont corrigés, chacun avec un test ; les points mineurs non traités vont dans [POINTS_REPORTES.md](POINTS_REPORTES.md).
4. **Compte rendu** : résultat, décisions prises et leur coût, ce qui a été vérifié (et comment), ce qui a échoué ou n'a pas pu être lancé, points reportés.

## 13. Actions qui demandent l'accord du responsable du projet

Un agent ne fait jamais ce qui suit sans accord explicite, donné pour cette action précise :

- Supprimer des données, des fichiers hors de son périmètre de travail, des conteneurs ou des volumes, ou toute autre action irréversible.
- Installer un logiciel ou un service sur la machine du développeur.
- Publier quoi que ce soit hors du dépôt local (`git push`, publication de paquet, d'image, de version, message sur un service externe).
- Modifier une décision acceptée (ADR), un seuil d'exigence (ENF-*) ou un critère de fin de tâche.
- Désactiver, contourner ou affaiblir une vérification (test, lint, banc, seuil de CI).
