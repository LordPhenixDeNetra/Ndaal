# Points reportés et décisions en attente — Ndaal

> Tout ce qui a été remis à plus tard est noté ici, pour ne jamais dépendre de la mémoire de quelqu'un ni d'un registre non versionné ([CLAUDE.md §9](CLAUDE.md)). Une entrée est close quand la décision est prise (ADR, exigence ou tâche mise à jour) ou le point traité ; elle reste alors dans le tableau avec son statut.

## Décisions en attente

| ID | Sujet | Options | Choix appliqué par défaut | À trancher avant | Statut |
|---|---|---|---|---|---|
| D-01 | Version minimale de Rust (MSRV) et version figée dans `rust-toolchain.toml` | Dernière version stable au démarrage du projet, ou une version plus ancienne pour élargir les utilisateurs de la crate `ndaal-client` | Aucun | T01.01 | Close le 2026-10-12 : Rust 1.99.0, édition 2024, MSRV 1.99 (`rust-toolchain.toml`) |
| D-02 | Registre des images de conteneur | Dépôt fixé le 2026-10-12 : `github.com/LordPhenixDeNetra/Ndaal`. Reste le registre d'images : GitHub Container Registry (`ghcr.io/lordphenixdenetra/ndaal`, gratuit pour un dépôt public, publication depuis GitHub Actions sans secret) ou Docker Hub (`ndaal/ndaal`, nom plus court, compte et jeton à gérer) | `ndaal/ndaal` (provisoire, dans [EXPLOITATION.md](EXPLOITATION.md)) | T01.22 | Ouvert (dépôt décidé) |
| D-03 | Règle de parité de performance (ENF-COMP) | Égaler les services remplacés (tolérance 5 %) ou les battre partout | Égaler, et battre sur la charge mixte RAG ([ADR-027](DECISIONS.md#adr-027--parité-de-performance-avec-les-services-remplacés)) | J07 | À confirmer |
| D-04 | Rôle de macOS et Windows | Développement et évaluation seulement, ou aussi production | Développement et évaluation, production sous Linux seulement (ENF-PORT-02, ENF-PORT-05) | J12 | À confirmer |
| D-05 | Machines dédiées aux bancs de performance (exécuteurs auto-hébergés, [ADR-028](DECISIONS.md#adr-028--cicd-sur-github-actions)) | Achat, location (serveurs dédiés) ou instances cloud réservées ; conformes à l'[environnement de référence](CAHIER_DES_CHARGES.md#7-environnement-de-référence), au moins deux machines (serveur et client) | Aucun | T01.18 | Ouvert |
| D-06 | Certificats de signature des binaires macOS (compte développeur Apple) et Windows (certificat Authenticode) | Les acheter (coût annuel) ou publier sans signature native, avec Sigstore seulement (avertissements de sécurité chez l'utilisateur) | Aucun | T12.19 | Ouvert |
| D-07 | Contact des signalements liés au code de conduite (`CODE_OF_CONDUCT.md`) | Adresse dédiée au projet (par exemple `conduct@` sur un domaine du projet), ou formulaire privé de sécurité de GitHub (détourné de son usage) | Formulaire privé de sécurité, titre commençant par « Code of conduct » | Arrivée du premier contributeur extérieur | Ouvert |
| D-08 | Approbations exigées pour fusionner dans `main` | 0 (demande de fusion et CI verte obligatoires) ou 1 (GitHub interdit d'approuver sa propre demande : bloquant tant qu'il n'y a qu'un mainteneur) | 0 approbation | Arrivée d'un second mainteneur | Ouvert |

## Points reportés

Remarques de revue non traitées, limites acceptées provisoirement, améliorations remises à plus tard.

| ID | Date | Origine | Point | Pourquoi reporté | Échéance ou condition | Statut |
|---|---|---|---|---|---|---|
| P-01 | 2026-10-12 | Revue finale J01 lot 1 | Racines de crates hors `src/lib.rs` et `src/main.rs` (`src/bin/*.rs`, `[[bin]] path` personnalisé) non vérifiées pour `forbid(unsafe_code)` | Aucun binaire de ce type n'existe encore | Premier `src/bin/` ajouté, ou lire les cibles via `cargo metadata` | Ouvert |
| P-02 | 2026-10-12 | Revue finale J01 lot 1 | Fichiers texte non UTF-8 (Latin-1, UTF-16) ignorés au lieu d'être signalés | Le plan les traitait comme binaires ; risque réel sous Windows PowerShell 5.1 | Avant T01.20 (CI) | Ouvert |
| P-03 | 2026-10-12 | Revue finale J01 lot 1 | Marque d'ordre des octets (BOM UTF-8) non signalée dans les fichiers Markdown | Déjà détectée dans `.rs`, `.toml`, `.yml` par la règle ASCII | Avant T01.20 (CI) | Ouvert |
| P-04 | 2026-10-12 | Revue finale J01 lot 1 | `check-linux.sh` monte les sources en écriture et sans `--locked` (fichier `Cargo.lock` possiblement modifié par root sur un hôte Linux) | Sans effet sous Windows et macOS | Avant le premier contributeur sous Linux | Ouvert |
| P-05 | 2026-10-12 | Revue finale J01 lot 1 | `check-deny.sh` affiche un message trompeur si `cargo-deny` n'est pas installé | `install-tools.sh` l'installe ; message à clarifier | Avant T01.20 (CI) | Ouvert |
| P-06 | 2026-10-12 | Revue finale J01 lot 1 | Fichier sans saut de ligne final signalé sans numéro de ligne | Le chemin suffit à le retrouver | Au besoin | Ouvert |
| P-07 | 2026-10-13 | Revue finale J01 lot 2 | L'exception Dependabot de `check-dco.sh` repose sur le nom d'auteur, falsifiable ; vérifier plutôt l'auteur de la demande de fusion (`github.event.pull_request.user.login`) | Le DCO est un engagement, pas un contrôle d'accès ; la relecture reste le garde-fou | Arrivée de contributeurs extérieurs | Ouvert |
| P-08 | 2026-10-13 | Revue finale J01 lot 2 | `cancel-in-progress` annule aussi les exécutions sur `main` lors de fusions rapprochées ; groupe de concurrence sans le nom du workflow | Un seul mainteneur, fusions espacées | Création de `nightly.yml` ou `docs.yml` | Ouvert |
| P-09 | 2026-10-13 | Revue finale J01 lot 2 | `persist-credentials: false` absent des étapes `checkout` | Jeton en lecture seule : impact faible | Avant tout workflow avec droits d'écriture (`release.yml`) | Ouvert |
| P-10 | 2026-10-13 | Revue finale J01 lot 2 | Le composant `llvm-tools-preview` est téléchargé par tous les travaux, alors que seul `coverage` l'utilise | Parité avec les postes locaux, coût de quelques dizaines de Mio | Si la durée de la CI devient un problème | Ouvert |
| P-11 | 2026-10-13 | Fusion de la demande n° 1 | Le commit `4af9874` (fusion « Squash ») a pour auteur l'adresse noreply de GitHub alors que ses signatures DCO portent une autre adresse | Réécrire `main` est interdit par les règles ; les fusions sont désormais limitées à « Rebase » | — | Close (accepté) |
| P-12 | 2026-10-13 | Épinglage d'Ubuntu | `macos-latest` et `windows-latest` restent flottants alors qu'`ubuntu-24.04` est épinglé | Seul Ubuntu avait une migration annoncée (Ubuntu 26 le 2026-10-19) | Prochaine migration annoncée d'une image macOS ou Windows | Ouvert |
