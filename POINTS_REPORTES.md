# Points reportés et décisions en attente — Ndaal

> Tout ce qui a été remis à plus tard est noté ici, pour ne jamais dépendre de la mémoire de quelqu'un ni d'un registre non versionné ([CLAUDE.md §9](CLAUDE.md)). Une entrée est close quand la décision est prise (ADR, exigence ou tâche mise à jour) ou le point traité ; elle reste alors dans le tableau avec son statut.

## Décisions en attente

| ID | Sujet | Options | Choix appliqué par défaut | À trancher avant | Statut |
|---|---|---|---|---|---|
| D-01 | Version minimale de Rust (MSRV) et version figée dans `rust-toolchain.toml` | Dernière version stable au démarrage du projet, ou une version plus ancienne pour élargir les utilisateurs de la crate `ndaal-client` | Aucun | T01.01 | Ouvert |
| D-02 | Registre des images de conteneur | Dépôt fixé le 2026-10-12 : `github.com/LordPhenixDeNetra/Ndaal`. Reste le registre d'images : GitHub Container Registry (`ghcr.io/lordphenixdenetra/ndaal`, gratuit pour un dépôt public, publication depuis GitHub Actions sans secret) ou Docker Hub (`ndaal/ndaal`, nom plus court, compte et jeton à gérer) | `ndaal/ndaal` (provisoire, dans [EXPLOITATION.md](EXPLOITATION.md)) | T01.22 | Ouvert (dépôt décidé) |
| D-03 | Règle de parité de performance (ENF-COMP) | Égaler les services remplacés (tolérance 5 %) ou les battre partout | Égaler, et battre sur la charge mixte RAG ([ADR-027](DECISIONS.md#adr-027--parité-de-performance-avec-les-services-remplacés)) | J07 | À confirmer |
| D-04 | Rôle de macOS et Windows | Développement et évaluation seulement, ou aussi production | Développement et évaluation, production sous Linux seulement (ENF-PORT-02, ENF-PORT-05) | J12 | À confirmer |
| D-05 | Machines dédiées aux bancs de performance (exécuteurs auto-hébergés, [ADR-028](DECISIONS.md#adr-028--cicd-sur-github-actions)) | Achat, location (serveurs dédiés) ou instances cloud réservées ; conformes à l'[environnement de référence](CAHIER_DES_CHARGES.md#7-environnement-de-référence), au moins deux machines (serveur et client) | Aucun | T01.18 | Ouvert |
| D-06 | Certificats de signature des binaires macOS (compte développeur Apple) et Windows (certificat Authenticode) | Les acheter (coût annuel) ou publier sans signature native, avec Sigstore seulement (avertissements de sécurité chez l'utilisateur) | Aucun | T12.19 | Ouvert |

## Points reportés

Remarques de revue non traitées, limites acceptées provisoirement, améliorations remises à plus tard.

| ID | Date | Origine | Point | Pourquoi reporté | Échéance ou condition | Statut |
|---|---|---|---|---|---|---|
| | | | | | | |
