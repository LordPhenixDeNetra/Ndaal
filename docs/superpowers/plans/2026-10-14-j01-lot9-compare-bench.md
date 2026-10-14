# J01 lot 9 — Banc comparatif (T01.18) : plan d'implémentation

> **Pour les agents :** sous-compétence requise : superpowers:executing-plans, tâche par tâche.
>
> **Pas d'étape de commit** : le responsable commite sur une branche `j01-lot9-compare-bench` et ouvre une demande de fusion ; la CI fait foi. Le compte rendu donne les commandes git **et le contenu du formulaire de demande de fusion**.

**Objectif :** l'infrastructure des bancs comparatifs `bench/compare/`.
- Elle lance tour à tour un système candidat et une référence, en conteneur, avec le même confinement CPU.
- Elle consigne les versions et les configurations.
- Elle compare les deux à la tolérance ENF-COMP et produit un rapport.
- Un écart au-delà de la tolérance ouvre un ticket.
- Elle tourne chaque nuit.

Exemple exigé par T01.18 : Valkey comparé à lui-même.

**Architecture :**
- `bench/compare/kv.sh` fait les passages : serveur et client `memtier_benchmark` en conteneurs, sur un réseau Docker dédié, avec des cœurs disjoints.
  - Les passages sont alternés (référence, candidat, référence, candidat...), 5 répétitions par défaut (STRATEGIE §9.1).
  - Il écrit un dossier de résultats : les JSON de memtier et `metadata.json`.
- `xtask compare` lit ce dossier, prend la médiane de chaque côté, applique la tolérance d'ADR-027, écrit `report.md` et sort en code 1 au-delà de la tolérance.
- Le workflow `compare.yml` tourne chaque nuit et à la demande, publie le rapport et ouvre ou commente un ticket.

**Pile :**
- `valkey/valkey:9.0.6@sha256:63346cb24a61221e76bdf41acce99b3968a9fa83d8122144deab45394b27b4f2` ;
- `redislabs/memtier_benchmark:2.5.2@sha256:389090e9ef13701f36f5ac97485742618e648adbf76c008418383594a05c1659` ;
- `serde_json` dans `xtask` uniquement.

**Spécification :**
- [CAHIER_DES_TACHES.md](../../../CAHIER_DES_TACHES.md) T01.18 ;
- [CAHIER_DES_CHARGES.md](../../../CAHIER_DES_CHARGES.md) O7, ENF-COMP ;
- [DECISIONS.md](../../../DECISIONS.md) ADR-027 (tolérance : débit ≥ 95 %, latence p99 ≤ 105 % de la référence) ;
- [STRATEGIE_DE_TESTS.md](../../../STRATEGIE_DE_TESTS.md) §9.1 et §9.4 (règles d'équité 1 à 7).

## Contraintes globales

- **Tolérance ADR-027.** Le candidat passe si son débit est au moins 95 % de celui de la référence **et** si sa latence p99 est au plus 105 % de celle de la référence. La limite exacte passe (95,0 % et 105,0 %).
- **Équité (STRATEGIE §9.4).**
  - Les passages se font tour à tour, jamais en même temps.
  - Le serveur et le client sont confinés à des cœurs disjoints (`--cpuset-cpus`), en paramètres du banc.
  - La durabilité est la même des deux côtés : `appendonly no` pour l'exemple.
  - Les caches du système sont vidés entre deux passages quand c'est possible (`sudo` sous Linux) ; sinon, le rapport le dit.
- **Versions consignées.** Image et empreinte, noyau, nombre de CPU, paramètres, date, commit de Ndaal.
- **Images épinglées par empreinte** (CLAUDE.md §4.12).
- **Ticket** : titre « compare: `<banc>` beyond tolerance », étiquette `perf`. Un ticket ouvert pour le même banc reçoit un commentaire. Permission `issues: write` dans le seul travail concerné.
- **Code** : aucun emoji, code en ASCII ; test d'abord.

## Décisions à valider avec le plan

1. **D-05 repoussée à J03** (accord donné) : sa colonne « À trancher avant » passe de T01.18 à « premiers bancs comparatifs de Ndaal (`cmp-kv`, J03) ».
2. **Machines partagées de GitHub en attendant** (`ubuntu-24.04`, 4 processeurs) : serveur sur le cœur 0, client sur les cœurs 2 et 3, 60 s par passage, 5 répétitions alternées. Leur bruit peut dépasser 5 % : la comparaison de Valkey à lui-même peut alors ouvrir un ticket qui est un faux positif. C'est un faux positif utile, puisqu'il mesure le bruit qui justifie D-05. Il est dédoublonné : un seul ticket, commenté. Point reporté P-50.
3. **Écart simulé** : une entrée `simulate_regression` du lancement manuel limite le candidat à 30 % d'un cœur (`--cpus 0.3`). C'est un vrai handicap, pas des chiffres falsifiés : il doit ouvrir le ticket (critère de fin de T01.18).
4. **Outil de charge `memtier_benchmark`** (STRATEGIE §9.4), en conteneur. C'est un outil externe exécuté, pas une dépendance liée, donc sa licence GPL n'entre pas dans le périmètre de C-02.
5. **`serde_json` dans `xtask`** pour lire la sortie JSON de memtier. C'est un outil de développement, pas une dépendance du moteur, donc il n'y a pas d'ADR.

## Points à surveiller en revue

1. **Débit à exactement 95 % ou latence p99 à exactement 105 %** : c'est un succès ; juste au-delà, c'est un échec. Testé à la tâche 1.
2. **Sortie memtier incomplète** (passage interrompu, champ absent) : erreur explicite (code 2) qui nomme le fichier, jamais une comparaison faite sur des zéros. Testé à la tâche 1.
3. **Nombre de répétitions inégal ou nul d'un côté** : erreur explicite. Testé à la tâche 1.
4. **Conteneur serveur qui ne démarre pas** : `kv.sh` échoue en moins de 30 s, avec un message clair, et nettoie ses conteneurs et son réseau (`trap`). Vérifié à la tâche 2.
5. **Machine avec moins de cœurs que demandé** : `kv.sh` refuse avant de lancer quoi que ce soit. Vérifié à la tâche 2.

---

### Tâche 1 : `xtask compare`

**Fichiers :**
- Créer :
  - `xtask/src/compare/{mod.rs, memtier.rs, verdict.rs}` ;
  - `xtask/tests/fixtures/memtier-2.5.2.json`, une vraie sortie, capturée à la tâche 2 et réduite aux champs lus.
- Modifier : `xtask/src/main.rs` (sous-commande `compare <dossier>`), `xtask/Cargo.toml`, `Cargo.toml` (`serde_json`).

**Interfaces :**
- `memtier::parse(json: &str) -> Result<Sample, String>` avec `Sample { ops_per_sec: f64, p99_ms: f64 }`. Les valeurs sont lues dans `"ALL STATS" > "Totals" > "Ops/sec"` et `"Percentile Latencies" > "p99.00"` : à confirmer sur la vraie sortie, et la fixture fait foi. Un champ absent, non numérique, négatif ou un débit nul est une erreur qui le nomme.
- `verdict::compare(reference: &[Sample], candidate: &[Sample]) -> Result<Verdict, String>` :
  - médianes de chaque côté ;
  - `Verdict { reference: Sample, candidate: Sample, throughput_ratio, p99_ratio, within_tolerance: bool }` ;
  - erreur si un côté est vide ou si les deux côtés n'ont pas le même nombre de répétitions.
- `MIN_THROUGHPUT_RATIO = 0.95`, `MAX_P99_RATIO = 1.05`, avec un commentaire qui renvoie à ADR-027.
- `report(bench: &str, metadata: &serde_json::Value, verdict: &Verdict) -> String` : rapport Markdown avec un tableau référence / candidat / ratio / seuil, le verdict, et les métadonnées recopiées (règle 6).
- `cargo run -p xtask -- compare <dossier>` :
  - lit `<dossier>/metadata.json`, `<dossier>/reference-*.json` et `<dossier>/candidate-*.json` ;
  - écrit `<dossier>/report.md` et l'affiche ;
  - code de sortie : 0 dans la tolérance, 1 au-delà, 2 si les entrées sont invalides.

- [ ] **Tests d'abord** :
  - `parses_memtier_output` : la fixture, avec les valeurs attendues ;
  - `missing_or_invalid_fields_are_errors` : un champ absent, une chaîne, un débit nul ;
  - `tolerance_boundaries` : un débit de 95,0 % passe et un de 94,9 % échoue ; une latence p99 de 105,0 % passe et une de 105,1 % échoue ;
  - `medians_resist_one_outlier` : 5 répétitions dont une aberrante ;
  - `unequal_or_empty_sides_are_errors` ;
  - `report_lists_values_ratios_and_metadata`.

---

### Tâche 2 : `bench/compare/kv.sh`

**Fichiers :** créer `bench/compare/kv.sh` et `bench/compare/valkey.conf` ; modifier `bench/README.md`.

**Interface :** `bench/compare/kv.sh [options]`, avec ses valeurs par défaut :
- `--server-cpus 0`, `--client-cpus 2,3` ;
- `--duration 60`, `--repeats 5` ;
- `--reference IMAGE` et `--candidate IMAGE`, Valkey épinglé par défaut des deux côtés ;
- `--candidate-cpu-quota Q`, vide par défaut ;
- `--out DIR`, par défaut `bench-results/kv-<date>`.

Comportement :
- **Mesure.** memtier lance `--protocol=redis --ratio=1:10 --data-size=100 --threads=2 --clients=25 --test-time=<duration> --json-out-file`, par passage. C'est la charge de `cmp-kv-latency` à débit libre, en version courte.
- **Démarrage.** Le script attend que le serveur réponde `PONG` (30 s au plus) avant de mesurer.
- **Nettoyage.** Un `trap` supprime les conteneurs et le réseau, même en cas d'échec.
- **Métadonnées.** `metadata.json` reçoit l'empreinte réelle de chaque image (`docker image inspect`), `uname -r`, `nproc`, les paramètres, la date UTC et `git rev-parse HEAD`.
- **Cœurs.** Le script refuse si un cœur demandé dépasse `nproc`.

- [ ] Vérification locale avec Docker, en version courte : `--duration 10 --repeats 2`, puis `xtask compare` sur le dossier. Deux essais :
  - Valkey contre Valkey, rapport produit ;
  - `--candidate-cpu-quota 0.3`, qui doit sortir en code 1.

  La sortie JSON de ce passage devient la fixture de la tâche 1 : on revient alors sur la tâche 1 si les noms de champs diffèrent.

---

### Tâche 3 : `compare.yml`

**Fichier :** `.github/workflows/compare.yml`.

- **Déclencheurs :**
  - `schedule` chaque nuit à 04:00 UTC, après `bench` ;
  - `workflow_dispatch` avec l'entrée booléenne `simulate_regression`.
- **Travail `kv`** sur `ubuntu-24.04` :
  1. `bench/compare/kv.sh --out results`, avec `--candidate-cpu-quota 0.3` si `simulate_regression` est vrai ;
  2. `cargo run -p xtask -- compare results` ;
  3. le rapport est ajouté au résumé (`$GITHUB_STEP_SUMMARY`) et le dossier publié en artefact (`compare-kv`, 30 jours) ;
  4. sur un écart, ticket « compare: kv beyond tolerance », étiquette `perf`, ouvert ou commenté, avec le lien vers l'exécution.
- [ ] Vérifications : `check-workflows.sh`. Après la fusion, deux lancements manuels :
  - un normal, qui produit un rapport ;
  - un avec `simulate_regression`, qui ouvre le ticket.

  C'est la preuve du critère de fin.

---

### Tâche 4 : documentation, suivi, vérifications

- [ ] Documents à mettre à jour :
  - STRATEGIE §9.4 : mise en œuvre actuelle ;
  - STRATEGIE §13.4 : ajout de `compare.yml` ;
  - `bench/README.md` ;
  - POINTS_REPORTES : D-05, avec sa nouvelle échéance, et P-50 (bruit des machines partagées).
- [ ] Chaîne complète de CLAUDE.md §11, dont `check-links.sh`, `check-linux.sh` et `check-workflows.sh` (Docker répond).
- [ ] Revue finale par un relecteur neuf.
- [ ] Compte rendu, avec les commandes git et le contenu du formulaire de demande de fusion.
- [ ] T01.18 n'est cochée qu'après les deux lancements manuels réussis sur `main`.
