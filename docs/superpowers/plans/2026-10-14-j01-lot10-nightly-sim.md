# J01 lot 10 — Simulation nocturne (T01.26) : plan d'implémentation

> **Pour les agents :** sous-compétence requise : superpowers:executing-plans, tâche par tâche.
>
> **Pas d'étape de commit** : le responsable commite sur une branche `j01-lot10-nightly-sim` et ouvre une demande de fusion. Le compte rendu donne les commandes git **et le contenu du formulaire de demande de fusion**.

**Objectif :** la simulation déterministe rejoue chaque nuit au moins 10 000 graines, et une graine qui échoue ouvre un ticket qui permet de la rejouer. C'est le critère 2 de passage d'un jalon (STRATEGIE §14), sans lequel J01 ne peut pas être clos.

**Architecture :**
- **Balayage de graines.** Le binaire `ndaal-sim` gagne une commande `sweep`, qui rejoue une plage de graines et vérifie, pour chacune :
  - qu'elle se termine (scénario complet, toutes les réponses reçues) ;
  - qu'elle est reproductible : deux exécutions de la même graine donnent la même empreinte.
- **Rapport.** La commande liste les graines fautives et sort en code 1 s'il y en a.
- **Workflow nocturne.** Le nouveau `nightly.yml`, prévu par T01.20 et STRATEGIE §13.4, lance ce balayage sur une plage différente chaque nuit, et ouvre ou commente un ticket qui donne la commande de rejeu.

**Spécification :**
- [STRATEGIE_DE_TESTS.md](../../../STRATEGIE_DE_TESTS.md) §4 (simulation), §13.2 (« Simulation : ≥ 10 000 graines » chaque nuit), §13.4 (`nightly.yml`), §14 critère 2 ;
- [CAHIER_DES_TACHES.md](../../../CAHIER_DES_TACHES.md) T01.09 (scénario `ping_pong`), T01.20 ;
- [POINTS_REPORTES.md](../../../POINTS_REPORTES.md) P-08.

## Contraintes globales

- **Graines.** Chaque nuit, la plage est différente pour ne pas rejouer les mêmes graines. La graine de départ est dérivée du numéro d'exécution (`run_number × 10 000`) et figure dans le résumé de l'exécution.
- **Ticket** « sim: failing seeds in ping_pong », étiquette `sim`. Il donne la commande de rejeu `cargo run -p ndaal-sim -- run --seed N` pour chaque graine fautive (au plus 20 listées) et la commande du balayage complet. Un ticket ouvert pour le même scénario reçoit un commentaire.
- **Délais.** Le workflow s'arrête au bout de 60 minutes. La boucle du simulateur est déjà bornée par `Limits` (pas et temps virtuel).
- **Permissions.** `contents: read`, et `issues: write` dans le seul travail concerné ; actions épinglées par empreinte.
- **Code.** Aucun emoji, code en ASCII, test d'abord.

## Décisions à valider avec le plan

1. **Nouvelle tâche T01.26** dans le cahier des tâches, comme le veut CLAUDE.md §9 pour une tâche découverte : « `nightly.yml` : simulation déterministe nocturne (≥ 10 000 graines), ticket sur graine fautive ». Elle est consignée dans le registre des modifications du cahier.
   - **Fin** : trois nuits consécutives vertes sur `main`.
   - **T01.20** est corrigée : son titre ne citera plus `nightly.yml`, qui passe à T01.26.
2. **Seul le scénario `ping_pong` existe.** Le balayage porte sur lui ; chaque scénario ajouté en J02 rejoindra `sweep`.
3. **P-08, résolu au passage**, comme son échéance le prévoyait :
   - `cancel-in-progress` ne vaut plus que pour les demandes de fusion, dans `ci.yml`, `docs.yml` et `fuzz.yml` (avec `${{ github.event_name == 'pull_request' }}`) ;
   - une exécution sur `main`, un déploiement ou un fuzzing nocturne n'est donc plus annulé ;
   - cela règle aussi une partie de P-45 et de P-48.
4. **Clôture de J01.** Après les trois nuits vertes, un petit commit de documentation remplit le journal des jalons de J01 et passe le jalon à « Terminé ».

## Points à surveiller en revue

1. **Graine qui ne se termine pas** (blocage, limite de pas atteinte) : elle est listée comme fautive, avec la raison ; le balayage continue jusqu'au bout. Testé à la tâche 1.
2. **Graine non reproductible** (empreintes différentes) : elle est fautive, et c'est le défaut le plus grave pour la simulation. Testé à la tâche 1, avec une fonction de scénario injectée.
3. **Plage qui déborde `u64`** (`--from` proche de `u64::MAX`) : erreur d'usage, pas de panique. Testé à la tâche 1.
4. **Plus de 20 graines fautives** : le ticket en liste 20, et le nombre total figure dans le résumé. Vérifié par relecture du workflow.

---

### Tâche 1 : `ndaal-sim sweep`

**Fichiers :** créer `crates/ndaal-sim/src/sweep.rs` ; modifier `crates/ndaal-sim/src/{lib.rs, main.rs}`.

**Interfaces :**
- `pub fn sweep(from: u64, count: u64, run: impl Fn(u64) -> SeedResult) -> Vec<Failure>`, avec :
  - `SeedResult { finished: bool, fingerprint: u64, detail: String }` ;
  - `Failure { seed: u64, reason: String }`.

  Le scénario est injecté, pour pouvoir tester les deux pannes. Chaque graine est jouée deux fois : une raison `"not reproducible"` si les empreintes diffèrent, sinon le `detail` si `finished` est faux.
- `pub fn ping_pong_result(seed: u64, messages: u32) -> SeedResult` : il réussit si `outcome == Finished` et `replies == messages`.
- La commande `ndaal-sim sweep --from <u64> --count <u64> [--messages <u32>]` :
  - affiche une ligne par graine fautive, `seed N: reason`, puis un résumé `swept N seeds, F failed` ;
  - sort en code 0 sans échec, 1 avec au moins un échec, 2 en cas d'erreur d'usage ;
  - refuse une plage qui déborde `u64` (code 2).

- [ ] **Tests d'abord :**
  - `sweep_reports_unfinished_seeds` : un scénario qui échoue sur les graines paires ;
  - `sweep_reports_non_reproducible_seeds` : un scénario dont l'empreinte change à chaque appel (compteur dans une `Cell`) ;
  - `sweep_of_ping_pong_is_clean` : 200 graines ;
  - `sweep_rejects_overflowing_range` et ses variantes dans l'analyse des arguments.

---

### Tâche 2 : `nightly.yml` et P-08

**Fichiers :** créer `.github/workflows/nightly.yml` ; modifier `.github/workflows/{ci.yml, docs.yml, fuzz.yml}`.

- **`nightly.yml`** :
  - déclencheurs : `schedule` chaque nuit à 01:00 UTC et `workflow_dispatch`, avec l'entrée `count` (10 000 par défaut) ;
  - travail `simulation` sur `ubuntu-24.04` : `cargo run --release -p ndaal-sim -- sweep --from <run_number × 10 000> --count <count>`, sortie copiée dans `$GITHUB_STEP_SUMMARY` ;
  - sur échec : artefact de la sortie, et ticket ouvert ou commenté avec les commandes de rejeu.
- **P-08** : `cancel-in-progress: ${{ github.event_name == 'pull_request' }}` dans `ci.yml`, `docs.yml` et `fuzz.yml`.
- [ ] Vérifications : `check-workflows.sh`, et un balayage local de 10 000 graines en `--release`, dont la durée est consignée.

---

### Tâche 3 : documentation, suivi, vérifications

- [ ] Documents à mettre à jour :
  - CAHIER_DES_TACHES : T01.26 ajoutée (non cochée), T01.20 corrigée, registre des modifications, tableau de bord (J01 : 25/26) ;
  - STRATEGIE §13.4 : ligne `nightly.yml` telle que réalisée ;
  - POINTS_REPORTES : P-08 clos ; P-45 et P-48 mis à jour.
- [ ] Chaîne complète de CLAUDE.md §11, dont `check-linux.sh` et `check-workflows.sh`.
- [ ] Revue finale par un relecteur neuf.
- [ ] Compte rendu avec les commandes git et le contenu du formulaire de demande de fusion.
- [ ] T01.26 n'est cochée qu'après trois nuits vertes sur `main`, puis J01 est clos (décision 4).
