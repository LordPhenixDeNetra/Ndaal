# Stratégie de tests — Ndaal

> Un moteur de données ne vaut que par la confiance qu'on peut lui accorder. Ce document décrit comment cette confiance est construite et maintenue. Exigences liées : ENF-DUR, ENF-QUAL, ENF-COMPAT, ENF-PERF, ENF-COMP, ENF-ISO, ENF-SEC-03.

## Sommaire

1. [Principes](#1-principes)
2. [Pyramide des tests](#2-pyramide-des-tests)
3. [Tests unitaires et de propriétés](#3-tests-unitaires-et-de-propriétés)
4. [Simulation déterministe](#4-simulation-déterministe)
5. [Fuzzing](#5-fuzzing)
6. [Tests de durabilité et de pannes réelles](#6-tests-de-durabilité-et-de-pannes-réelles)
7. [Vérification de cohérence](#7-vérification-de-cohérence)
8. [Tests de compatibilité](#8-tests-de-compatibilité)
9. [Bancs d'essai de performance](#9-bancs-dessai-de-performance)
10. [Tests d'isolation](#10-tests-disolation)
11. [Tests de bout en bout](#11-tests-de-bout-en-bout)
12. [Tests de sécurité](#12-tests-de-sécurité)
13. [Intégration continue](#13-intégration-continue)
14. [Critères de passage d'un jalon](#14-critères-de-passage-dun-jalon)

---

## 1. Principes

1. **Reproductible ou inutile.** Tout test qui échoue doit pouvoir être rejoué à l'identique (graine, entrée de fuzzing, trace).
2. **Chercher les bogues, pas la couverture.** La couverture est un indicateur (ENF-QUAL-01), pas un objectif : la simulation et le fuzzing trouvent les bogues qui comptent.
3. **Un bogue = un test.** Toute correction ajoute un test qui échouait avant (ENF-QUAL-04), y compris la graine de simulation qui l'a révélé.
4. **Les vrais clients jugent la compatibilité.** On teste avec `redis-py`, `boto3`, `aws-cli`, etc., pas seulement avec nos propres clients.
5. **La performance est une fonctionnalité.** Les régressions de performance bloquent l'intégration comme des régressions fonctionnelles.
6. **Se mesurer aux services remplacés, pas seulement à soi-même.** Chaque banc important tourne aussi contre Valkey, Qdrant ou MinIO sur la même machine (§9.4). Être plus lent qu'eux est un défaut, pas une limitation.

---

## 2. Pyramide des tests

| Niveau | Volume | Durée | Fréquence | Outils |
|---|---|---|---|---|
| Unitaires | Milliers | Secondes | Chaque commit | `cargo test`, `cargo nextest` |
| Propriétés | Centaines | Minutes | Chaque commit | `proptest` |
| Modèle (comparaison à une référence) | Dizaines | Minutes | Chaque commit | `proptest` + modèles en mémoire |
| Simulation déterministe | Milliers de graines | Minutes (CI) / heures (nuit) | Chaque commit + nuit | `ndaal-sim` |
| Fuzzing | Continu | Continu | Nuit + continu | `cargo fuzz` (libFuzzer) |
| Compatibilité | Centaines | Minutes | Chaque fusion | Conteneurs avec vrais clients |
| Intégration multi-nœuds | Dizaines | Minutes | Chaque fusion | Conteneurs, réseau réel |
| Pannes réelles | Dizaines de scénarios | Heures | Nuit / hebdomadaire | Machines virtuelles, coupures, `dm-flakey` |
| Performance | Dizaines de bancs | Heures | Nuit + avant publication | `ndaal bench`, `memtier_benchmark`, `warp`, bancs ANN |
| Comparatifs (parité) | Une quinzaine de bancs × 3 références | Heures | Nuit (version courte) + fin de jalon + avant publication | Mêmes outils, Valkey, Qdrant, MinIO/RustFS en conteneurs, VectorDBBench |
| Bout en bout | Quelques scénarios | Minutes | Chaque fusion | Application RAG de référence |

---

## 3. Tests unitaires et de propriétés

### 3.1 Règles

- Chaque crate a ses tests unitaires dans le même fichier (`#[cfg(test)]`) ou dans `tests/`.
- Les structures de données critiques (tables clé-valeur, encodages compacts, roue temporelle, SST, HNSW, bitmaps de suppression, index de métadonnées objet) ont des **tests de modèle** : une suite d'opérations aléatoires est appliquée à l'implémentation et à un modèle naïf (`BTreeMap`, `Vec`), et les résultats sont comparés après chaque opération.

### 3.2 Propriétés vérifiées (exemples)

| Composant | Propriété |
|---|---|
| Encodage des clés internes | `decode(encode(x)) == x` ; l'ordre des octets encodés respecte l'ordre logique |
| Journal | Toute séquence d'enregistrements écrite puis relue est identique ; une troncature à n'importe quel octet donne un préfixe valide |
| SST | Toute clé écrite est retrouvée ; aucune clé non écrite n'est retrouvée ; le parcours est trié |
| Effets clé-valeur | Appliquer une suite d'effets deux fois (reprise) donne le même état qu'une fois |
| Encodages compacts | La bascule petit ↔ grand encodage ne change aucun résultat de commande |
| HNSW | Rappel@10 ≥ seuil sur des données aléatoires ; une recherche exhaustive et une recherche HNSW avec `ef` = n donnent le même résultat |
| Filtres vectoriels | Résultat du filtre via index == évaluation directe sur la charge utile |
| Reed-Solomon | Toute combinaison de *k* fragments reconstruit le bloc |
| Multipart | ETag multipart identique au calcul d'AWS pour des tailles de parties aléatoires |
| Préconditions | Une transaction dont une précondition échoue ne modifie rien |

---

## 4. Simulation déterministe

### 4.1 Principe

`ndaal-sim` exécute **un cluster complet** (1 à 7 nœuds, avec leurs shards, leurs moteurs et leurs frontaux internes) dans **un seul thread**, en remplaçant toutes les sources de non-déterminisme par des implémentations simulées pilotées par une graine :

| Abstraction | Implémentation simulée |
|---|---|
| Horloge | Temps virtuel avancé par l'ordonnanceur ; dérive d'horloge par nœud injectable |
| Aléa | Générateur à graine unique |
| Disque | Système de fichiers en mémoire modélisant le cache de pages : les écritures non synchronisées peuvent être perdues, réordonnées ou partiellement écrites lors d'une coupure |
| Réseau | Connexions à trames, comme le protocole interne TCP (ADR-014) : latence aléatoire avec ordre conservé par connexion, connexions coupées ou refusées, partitions, nœuds arrêtés. Les pertes, duplications et réordonnancements de messages apparaissent au niveau des requêtes : trames perdues avec une connexion coupée, requêtes rejouées par les réessais, ordre différent entre connexions. Implémentation : `ndaal-io::sim::SimNetwork`, ordonnanceur : `ndaal-sim` |
| Ordonnancement | Ordre d'exécution des tâches prêtes choisi par la graine |

### 4.2 Pannes injectées

- Coupure de courant d'un ou plusieurs nœuds (perte de tout ce qui n'est pas synchronisé).
- Redémarrage propre.
- Disque : erreur d'E/S, écriture interrompue (*torn write*), corruption silencieuse d'un bloc, disque plein, latence extrême.
- Réseau : partition (symétrique ; asymétrique à venir, voir POINTS_REPORTES.md), latence élevée, coupure de connexion, nœud arrêté.
- Horloge : saut en avant ou en arrière, dérive.
- Opérations de cluster pendant les pannes : ajout et retrait de nœuds, division de partitions, transferts de leadership.

### 4.3 Charge de travail et vérifications

Des **clients simulés** exécutent des charges mixtes (clé-valeur, vecteurs, objets, transactions) et enregistrent l'**historique** de leurs opérations (invocation, réponse, horodatages virtuels). À la fin de chaque simulation :

1. **Aucune écriture acquittée n'est perdue** (ENF-DUR-01/02).
2. **Linéarisabilité** des opérations clé-valeur sur une clé (vérification par un vérificateur de type Knossos/Porcupine).
3. **Sérialisabilité** des transactions (graphe de dépendances sans cycle).
4. **Atomicité** : pour chaque transaction, soit tous ses effets sont visibles, soit aucun.
5. **Convergence** : tous les réplicas d'une partition ont le même état après stabilisation (comparaison d'empreintes).
6. **Invariants internes** : pas de bloc référencé manquant, pas de segment orphelin dans le manifeste, compteurs cohérents.
7. **Absence de blocage** : le cluster redevient disponible après la fin des pannes, dans un délai virtuel borné.

### 4.4 Volume

- CI sur chaque demande de fusion : 1 000 graines courtes.
- Nuit : ≥ 10 000 graines (ENF-QUAL-05), dont des simulations longues (heures de temps virtuel).
- Une graine qui échoue est conservée dans `tests/sim-regressions/` et rejouée à chaque commit.

---

## 5. Fuzzing

| Cible | Entrée | Vérification |
|---|---|---|
| `fuzz_resp_decoder` | Octets arbitraires | Pas de panique, mémoire bornée, résultat identique au décodage en un seul tampon et en morceaux |
| `fuzz_resp_commands` | Suites de commandes structurées | Pas de panique ; comparaison avec un modèle pour un sous-ensemble |
| `fuzz_s3_requests` | Requêtes HTTP S3 (en-têtes, XML, chemins) | Pas de panique ; erreurs S3 valides |
| `fuzz_sigv4` | En-têtes et requêtes présignées | Pas d'acceptation de signature invalide |
| `fuzz_http_json` | Corps JSON des API vectorielle et transactionnelle | Pas de panique, limites respectées |
| `fuzz_filter` | Filtres vectoriels | Résultat identique entre index et évaluation directe |
| `fuzz_wal_reader` | Fichiers de journal corrompus | Détection de corruption, jamais de donnée invalide appliquée |
| `fuzz_segment_reader` | Segments corrompus (tous types) | Erreur de somme de contrôle ou de format, jamais de panique |
| `fuzz_manifest` | Manifestes corrompus | Erreur explicite |
| `fuzz_internal_rpc` | Trames du protocole interne | Pas de panique |

Objectif : ≥ 1 milliard d'exécutions cumulées par cible avant la publication stable (ENF-SEC-03). Les corpus sont versionnés.

**Mise en œuvre.**
- Le corps de chaque cible est une fonction de la crate `ndaal-fuzz`, en Rust stable. Le corpus versionné (`fuzz/corpus/<cible>/`) y est rejoué par `cargo test` à chaque demande de fusion, sur les trois systèmes.
- Le dossier `fuzz/` est un workspace `cargo fuzz` séparé, dont chaque cible appelle ce corps. Il se construit avec une nightly figée (`fuzz/rust-toolchain.toml`), sous Linux ou dans le conteneur de `scripts/fuzz.sh`.
- Cibles en place : `common_codec` (décodeurs de `ndaal-common`). Les cibles du tableau ci-dessus arrivent avec leurs décodeurs (P-44).

---

## 6. Tests de durabilité et de pannes réelles

La simulation ne remplace pas le matériel réel.

| Scénario | Méthode |
|---|---|
| Coupure de courant | Machines virtuelles arrêtées brutalement (`virsh destroy`) pendant une charge d'écriture ; 10 000 cycles automatisés (ENF-DUR-01) |
| Écritures perdues par le disque | `dm-flakey` / `dm-log-writes` pour rejouer des états intermédiaires du disque |
| Corruption | Inversion de bits dans les fichiers de segments et de journal ; la donnée corrompue ne doit jamais être servie |
| Disque plein | Remplissage du système de fichiers ; vérification du comportement de §18 de l'architecture |
| Panne de nœud en cluster | Arrêt brutal de conteneurs/VM ; mesure du temps de basculement (ENF-DISPO-02) |
| Partition réseau réelle | `iptables` / `tc netem` |
| Mise à jour progressive | Mise à jour d'un cluster de 5 nœuds sous charge (ENF-DISPO-03) |
| Perte d'une zone | Arrêt de tous les nœuds d'une zone |

---

## 7. Vérification de cohérence

Une campagne inspirée de Jepsen est menée avant la publication stable et après tout changement du consensus ou des transactions :

- clients réels concurrents sur un cluster de 5 nœuds ;
- injecteur de pannes (partitions, arrêts, pauses de processus `SIGSTOP`, dérive d'horloge) ;
- charges : registres (linéarisabilité), compteurs (`INCR`), ensembles (aucune perte), files (`LPUSH`/`RPOP`, Streams), transactions de transfert entre comptes (sérialisabilité, conservation de la somme), transactions multi-modèles (atomicité objet + vecteur + clé) ;
- analyse avec un vérificateur (Elle pour les transactions, Knossos/Porcupine pour la linéarisabilité).

---

## 8. Tests de compatibilité

### 8.1 Redis (`tests/compat-redis/`)

- Suites de tests exécutées avec chaque client de ENF-COMPAT-01, en conteneurs.
- Un sous-ensemble de la suite de tests publique du projet Redis (licence permissive des versions concernées à vérifier) adapté au périmètre supporté, ou une suite équivalente écrite à partir de la documentation.
- `redis-benchmark` et `memtier_benchmark` doivent fonctionner sans erreur.
- Vérification automatique : chaque commande de la table renvoie le même type de réponse que Redis pour un jeu d'appels de référence (tests « différentiels » contre une instance Valkey de référence lancée en conteneur).

### 8.2 S3 (`tests/compat-s3/`)

- Suite `ceph/s3-tests` filtrée par les opérations supportées (ENF-COMPAT-03 : ≥ 90 %).
- Scénarios par client (ENF-COMPAT-02) : création de compartiment, envoi simple et multipart, listages avec préfixes, URL présignées, copie, étiquettes, versions.
- `rclone check` et `rclone sync` sur un jeu de 100 000 fichiers.
- `warp` (banc S3) sans erreur.

### 8.3 Vecteurs

- Tests de l'API contre la documentation ; SDK officiels testés contre le serveur.
- Bancs de rappel sur jeux publics (SIFT-1M, GloVe-100, un jeu de dimension 768 généré par un modèle d'embedding), avec rappel publié.

---

## 9. Bancs d'essai de performance

### 9.1 Méthode

- Exécution sur l'[environnement de référence](CAHIER_DES_CHARGES.md#7-environnement-de-référence), machines dédiées, fréquence CPU fixée.
- Chaque mesure : préchauffage, 5 répétitions, médiane et dispersion publiées.
- Les latences sont mesurées côté client **et** côté serveur (histogrammes HDR), en boucle ouverte (débit fixe) pour éviter l'omission coordonnée.

### 9.2 Bancs

| Banc | Outil | Exigence |
|---|---|---|
| `kv-latency` | `memtier_benchmark`, débit fixe | ENF-PERF-01 |
| `kv-throughput` | `memtier_benchmark`, pipeline, 1→16 cœurs | ENF-PERF-02 |
| `kv-replicated` | `ndaal bench kv --durability replicated` | ENF-PERF-03 |
| `vec-hnsw` | `ndaal bench vector`, 1 M × 768 | ENF-PERF-04 |
| `vec-disk` | 100 M × 768 | ENF-PERF-05 |
| `vec-ingest` | Ingestion par lots | ENF-PERF-06 |
| `obj-large` | `warp put/get`, objets 64 Mio–1 Gio | ENF-PERF-07 |
| `obj-small` | `warp`, objets 4–64 Kio | ENF-PERF-08 |
| `kv-pipeline` | `memtier_benchmark`, pipeline 16, 1 cœur | ENF-PERF-11 |
| `restart` | Redémarrage avec 10 Gio de journal | ENF-PERF-09 |
| `tx-overhead` | Transactions UC-01 vs opérations séparées | ENF-PERF-10 |
| `memory-overhead` | 100 M petites clés | ENF-RES-05 |
| `scale-out` | 1 → 32 nœuds | ENF-SCAL-06 |

### 9.3 Suivi des régressions

- Bancs courts (5 minutes) chaque nuit ; résultats stockés et tracés.
- Une régression > 5 % sur un banc clé ouvre automatiquement un ticket ; > 10 % bloque la publication.
- Micro-bancs (`criterion`) pour les noyaux critiques : distances SIMD, décodeur RESP, encodage des effets, recherche dans les blocs SST.

### 9.4 Bancs comparatifs (parité avec les services remplacés)

Ces bancs vérifient les exigences [ENF-COMP](CAHIER_DES_CHARGES.md#612-parité-avec-les-services-remplacés-enf-comp). Ils se trouvent dans `bench/compare/` : un script par banc, qui lance Ndaal **et** la référence, l'un après l'autre, sur la même machine.

**Règles d'équité** (une mesure qui ne les respecte pas est invalide) :

1. **Même matériel, même client, même charge.** La référence et Ndaal tournent tour à tour sur la machine de référence, jamais en même temps ; caches du système vidés entre deux passages.
2. **Même budget CPU.** Les deux processus sont confinés aux mêmes cœurs (`taskset`/cgroups) ; le nombre de cœurs est un paramètre du banc.
3. **Durabilité équivalente.** Ndaal `none`/`async`/`sync` contre Valkey `appendonly no`/`appendfsync everysec`/`appendfsync always` ; pour les objets, `fsync` avant acquittement des deux côtés ; pour les vecteurs, journal actif des deux côtés.
4. **Vecteurs : comparaison à rappel égal.** On trace la courbe débit/rappel de chaque système en faisant varier `ef`, puis on compare le débit et la latence p99 aux points de rappel 0,95 et 0,99.
5. **Référence bien réglée.** Configuration de production recommandée par l'éditeur de la référence, publiée avec les résultats ; un réglage qui l'avantage est accepté s'il est documenté.
6. **Versions consignées.** Version et empreinte de l'image de la référence, version de Ndaal, noyau, paramètres : tout est stocké avec le résultat.
7. **Reproductible par un tiers.** Les scripts et configurations sont publiés ; n'importe qui peut relancer la comparaison.

| Banc | Outil | Référence | Exigence |
|---|---|---|---|
| `cmp-kv-latency` | `memtier_benchmark` en boucle ouverte, 100 o et 1 Kio, 1:10 et 1:1 | Valkey (`io-threads` = cœurs) | ENF-COMP-01 |
| `cmp-kv-pipeline` | `memtier_benchmark`, pipeline 16 et 64, chaînes, hash, listes, ensembles triés | Valkey | ENF-COMP-02 |
| `cmp-kv-machine` | `memtier_benchmark --cluster-mode`, 16 cœurs | Valkey en cluster de 16 instances | ENF-COMP-03 |
| `cmp-vec-search` | VectorDBBench et bancs ANN (1 M et 10 M × 768, SIFT-1M, GloVe-100), avec et sans filtres | Qdrant | ENF-COMP-04 |
| `cmp-vec-ingest` | VectorDBBench, ingestion jusqu'à index prêt, mémoire résidente | Qdrant | ENF-COMP-05 |
| `cmp-obj-large` | `warp put/get`, 64 Mio–1 Gio, 1 nœud puis 4 nœuds | MinIO ou RustFS | ENF-COMP-06 |
| `cmp-obj-small` | `warp put/get/list`, 4–64 Kio | MinIO ou RustFS | ENF-COMP-07 |
| `cmp-rag-mixed` | Application RAG de référence (§11) en charge | Valkey + Qdrant + MinIO | ENF-COMP-08 |

**Fréquence et seuils** :

- **Chaque nuit** : version courte (5 minutes par banc, 1 et 16 cœurs). Un écart défavorable au-delà de la tolérance d'ENF-COMP ouvre automatiquement un ticket de priorité haute.
- **Fin de jalon** : version complète des bancs liés au jalon ; résultats et graphiques consignés dans le [journal des jalons](CAHIER_DES_TACHES.md#journal-des-jalons).
- **Avant publication** : version complète de tous les bancs ; **tout écart défavorable bloque la publication**, sans dérogation possible.
- Les résultats sont publiés tels quels, y compris lorsqu'ils sont défavorables à Ndaal.

---

## 10. Tests d'isolation

| Test | Charge de fond | Mesure | Exigence |
|---|---|---|---|
| `iso-transfer` | `warp put` saturant les disques | Latence p99 de `memtier` à débit fixe | ENF-ISO-01 |
| `iso-index` | Construction d'index de 10 M vecteurs | Latence p99 clé-valeur | ENF-ISO-02 |
| `iso-ingest` | Ingestion vectorielle massive | Latence p99 de recherche | ENF-ISO-03 |
| `iso-tenant` | Un locataire au-delà de son quota | Latence p99 des autres locataires | ENF-ISO-04 |
| `iso-overload` | Charge à 3× la capacité | Mémoire bornée, erreurs « occupé », reprise après la charge | ENF-ISO-05 |
| `iso-compaction` | Compactage forcé | Latence p99 clé-valeur et recherche | ENF-ISO-01/02 |

---

## 11. Tests de bout en bout

**Application RAG de référence** (`tests/e2e/rag/`) :

1. Ingestion de 10 000 documents PDF (objets provisoires + transactions UC-01).
2. Recherches avec filtres par locataire et cache (UC-02).
3. Suppression conforme de 1 000 documents (UC-03) : vérification qu'aucun vecteur, aucune clé de cache et aucun bloc ne subsiste (après la marge du ramasse-miettes).
4. Exécutée sur nœud unique, sur cluster de 3 nœuds, et en mode désagrégé avec un stockage objet externe (MinIO ou RustFS en conteneur pour le test).
5. Pannes injectées pendant l'ingestion : l'état final ne contient que des documents complets.

**Console web** (`console/tests/`, ENF-UI) :

1. Un scénario Playwright par écran (tableau de bord, clés, console RESP, collections, recherche, compartiments, administration), exécuté sur Chromium, Firefox et WebKit.
2. Scénario UC-01 construit, simulé (`dry_run`) puis exécuté depuis l'éditeur de transactions (EF-OPS-14).
3. Analyse d'accessibilité automatique (axe) sur chaque écran, en thème clair et sombre : aucune violation tolérée.
4. Sécurité : session expirée, jeton CSRF absent ou faux, utilisateur sans droit sur un espace de noms (chaque action doit être refusée par l'API, pas seulement masquée), en-têtes de sécurité présents.
5. Budget : taille des fichiers produits et temps de chargement initial (ENF-UI-03).

---

## 12. Tests de sécurité

- Fuzzing des entrées (§5).
- Matrice d'autorisation : pour chaque rôle type et chaque opération de chaque protocole, résultat attendu (autorisé/refusé) vérifié automatiquement.
- Tests SigV4 : signatures expirées, rejouées, mal formées, en-têtes non signés modifiés.
- Analyse TLS (versions, suites, certificats).
- Analyse statique : `cargo audit`, `cargo deny`, `cargo geiger` (inventaire `unsafe`), Miri sur les modules `unsafe`.
- Audit externe avant la publication stable (critère d'acceptation n° 7).

---

## 13. Intégration continue

### 13.1 À chaque demande de fusion

1. `cargo fmt --check`, `cargo clippy -- -D warnings`
2. `cargo deny check` (licences, avis de sécurité, sources)
3. Lint maison : interdiction des accès directs à l'horloge, au disque, au réseau et à l'aléa hors `ndaal-io`
4. Tests unitaires, propriétés et modèles (`cargo nextest`)
5. Simulation déterministe : 1 000 graines
6. Tests de compatibilité Redis et S3 (sous-ensemble rapide)
7. Vérification de la MSRV
8. Compilation **et suite de tests complète** sur Linux x86_64/aarch64, macOS aarch64 et Windows x86_64 (ENF-PORT-02)
9. Couverture (rapport, seuils ENF-QUAL-01 vérifiés)
10. Complétude des traductions et absence de texte humain écrit en dur dans la CLI et la console (ENF-I18N-05)
11. Construction de l'image de conteneur, démarrage via `docker compose` (nœud unique) et analyse de vulnérabilités de l'image
12. Hygiène du code : taille des fichiers, emojis, caractères non ASCII ([CLAUDE.md §4.6 et §4.7](CLAUDE.md))
13. Console : vérification des types (`svelte-check`, TypeScript strict), lint, tests unitaires, scénarios Playwright, analyse d'accessibilité, budget de taille, audit des dépendances npm (ENF-UI)

### 13.2 Chaque nuit

- Simulation : ≥ 10 000 graines
- Fuzzing : 1 h par cible (en plus du fuzzing continu)
- Compatibilité complète (`ceph/s3-tests`, tous les clients)
- Bancs courts de performance et d'isolation
- Bancs comparatifs courts contre Valkey, Qdrant et MinIO (§9.4)
- Tests d'intégration multi-nœuds et de pannes réelles courts
- Miri sur les modules `unsafe`

### 13.3 Avant chaque publication

- Bancs complets de performance, publiés
- Bancs comparatifs complets (§9.4) : tout écart défavorable à Ndaal bloque la publication
- Campagne de pannes réelles complète
- Mise à jour progressive depuis la publication précédente
- Lecture du corpus de formats des publications précédentes (ENF-COMPAT-04)
- Publication automatique (EF-OPS-13) : binaires des trois systèmes, images, chart Helm, crates, notes de version, signatures, SBOM et attestations de provenance

### 13.4 Mise en œuvre sur GitHub Actions

Décision : [ADR-028](DECISIONS.md#adr-028--cicd-sur-github-actions). Chaque workflow appelle des scripts du dépôt (`scripts/`), afin que tout puisse être relancé en local.

| Workflow | Déclencheur | Contenu |
|---|---|---|
| `ci.yml` | Demande de fusion, poussée sur `main` | §13.1 ; matrice `ubuntu-24.04`, `ubuntu-24.04-arm`, `macos-latest`, `windows-latest` ; travail `docker` (construction de l'image, `docker compose up --wait`, `/health/ready`, arrêt sur SIGTERM) sur `ubuntu-24.04` ; travail `console` (Node.js, Playwright) sur `ubuntu-latest` |
| `nightly.yml` | Planifié, chaque nuit | §13.2 (hors bancs) |
| `bench.yml` | Planifié chaque nuit, et à la demande | Bancs de performance, d'isolation et comparatifs (§9, §10) sur exécuteurs auto-hébergés dédiés. Aujourd'hui : micro-bancs `criterion` sur `ubuntu-24.04`, historique et graphe sur la branche `bench-data`, sans alerte en attendant D-05 (P-42) |
| `fuzz.yml` | Planifié chaque nuit, à la demande, et sur une demande de fusion qui touche `fuzz/` ou `ndaal-fuzz` | Fuzzing nocturne (1 h par cible ; 60 s sur une demande de fusion), corpus conservé dans le cache et publié en artefact, ticket « fuzz: crash in <cible> » ouvert ou commenté en cas de plantage |
| `docker.yml` | Fusion sur `main`, étiquette de version | Images multi-architectures, analyse de vulnérabilités, signature, publication |
| `release.yml` | Étiquette `v*` | §13.3, puis construction, signature et publication de tous les artefacts |
| `docs.yml` | Demande de fusion, fusion sur `main`, chaque nuit | Vérification des liens (`scripts/check-links.sh`, aussi dans `ci.yml`) et construction du site mdBook ; sur `main` et la nuit, déploiement sur GitHub Pages avec le graphe des bancs sous `/bench/` |

**Règles** :

- Actions tierces épinglées par **empreinte de commit** (pas par étiquette), mises à jour par Dependabot, comme les dépendances Cargo.
- Permissions du jeton `contents: read` par défaut ; élargies au seul travail qui en a besoin.
- Aucun secret accessible aux demandes de fusion venant de forks ; signatures sans clé (OIDC + Sigstore).
- Cache de compilation (`Swatinem/rust-cache`, `sccache`) ; `concurrency` annule les exécutions obsolètes d'une même branche.
- Bancs de performance **uniquement** sur exécuteurs auto-hébergés (environnement de référence) : les exécuteurs partagés sont trop bruités.
- Branche `main` protégée : workflows requis verts, une relecture, historique linéaire, signature DCO.
- Durée cible de `ci.yml` : moins de 20 minutes ; au-delà, paralléliser ou déplacer des vérifications vers `nightly.yml`.

---

## 14. Critères de passage d'un jalon

Un jalon du [cahier des tâches](CAHIER_DES_TACHES.md) n'est **terminé** que si :

1. Toutes ses tâches sont cochées avec leurs critères de fin vérifiés.
2. La CI est verte, y compris la simulation déterministe nocturne sur 3 nuits consécutives.
3. Les nouveaux composants ont leurs tests de propriétés, leurs cibles de fuzzing (s'ils décodent des entrées externes ou des formats) et leurs scénarios de simulation.
4. Les bancs liés au jalon sont exécutés et leurs résultats consignés dans la section « Journal des jalons » du cahier des tâches.
5. Les bancs comparatifs liés au jalon (§9.4) sont exécutés. À partir de J07, ils DOIVENT montrer la parité (ENF-COMP) ; avant J07, un écart est toléré mais il est consigné avec un plan pour le résorber en J07.
6. La documentation (architecture, API, exploitation) est à jour.
