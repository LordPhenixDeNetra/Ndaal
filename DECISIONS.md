# Registre des décisions d'architecture (ADR)

> Chaque décision importante est consignée ici. Une décision n'est jamais effacée : si elle change, une nouvelle ADR la **remplace** et l'ancienne passe au statut « Remplacée par ADR-xxx ».
>
> Statuts possibles : **Proposée**, **Acceptée**, **Remplacée**, **Abandonnée**.

## Modèle

```markdown
## ADR-nnn — Titre

- **Statut** : Proposée | Acceptée | Remplacée par ADR-xxx | Abandonnée
- **Date** : AAAA-MM-JJ
- **Exigences liées** : EF-…, ENF-…

### Contexte
### Options étudiées
### Décision
### Conséquences
```

## Index

| ADR | Titre | Statut |
|---|---|---|
| [001](#adr-001--licence-apache-20) | Licence Apache 2.0 | Acceptée |
| [002](#adr-002--rust-comme-unique-langage-du-serveur) | Rust comme unique langage du serveur | Acceptée |
| [003](#adr-003--thread-per-core-avec-tokio-en-mode-current_thread) | Thread-per-core avec `tokio` en mode `current_thread` | Acceptée |
| [004](#adr-004--un-journal-unique-multiplexé-par-shard) | Un journal unique multiplexé par shard | Acceptée |
| [005](#adr-005--lespace-de-noms-comme-unité-de-transaction-et-de-placement) | L'espace de noms comme unité de transaction et de placement | Acceptée |
| [006](#adr-006--pas-de-transactions-entre-espaces-de-noms) | Pas de transactions entre espaces de noms | Acceptée |
| [007](#adr-007--journalisation-logique-idempotente-par-version) | Journalisation logique idempotente par version | Acceptée |
| [008](#adr-008--raft-par-partition-avec-openraft) | Raft par partition avec `openraft` | Acceptée |
| [009](#adr-009--stockage-objet-comme-fondation-optionnelle) | Stockage objet comme fondation optionnelle | Acceptée |
| [010](#adr-010--octets-des-objets-hors-du-journal) | Octets des objets hors du journal | Acceptée |
| [011](#adr-011--implémentation-hnsw-propre-en-rust) | Implémentation HNSW propre en Rust | Acceptée |
| [012](#adr-012--index-disque-de-type-vamanadiskann) | Index disque de type Vamana/DiskANN | Acceptée |
| [013](#adr-013--couche-s3-basée-sur-s3s) | Couche S3 basée sur `s3s` | Acceptée |
| [014](#adr-014--protocole-interne-binaire-léger) | Protocole interne binaire léger | Acceptée |
| [015](#adr-015--protocol-buffers-pour-les-métadonnées-évolutives) | Protocol Buffers pour les métadonnées évolutives | Acceptée |
| [016](#adr-016--abstraction-des-es-avec-deux-implémentations) | Abstraction des E/S avec deux implémentations | Acceptée |
| [017](#adr-017--simulation-déterministe) | Simulation déterministe | Acceptée |
| [018](#adr-018--compression-lz4-par-défaut) | Compression LZ4 par défaut | Acceptée |
| [019](#adr-019--allocateur-mémoire) | Allocateur mémoire | Acceptée |
| [020](#adr-020--exception-cryptographique-ring) | Exception cryptographique `ring` | Acceptée |
| [021](#adr-021--cache-hybride-foyer) | Cache hybride `foyer` | Acceptée |
| [022](#adr-022--ramasse-miettes-des-blocs-par-marquage-balayage) | Ramasse-miettes des blocs par marquage-balayage | Acceptée |
| [023](#adr-023--plan-de-contrôle-dans-un-groupe-raft-système) | Plan de contrôle dans un groupe Raft système | Acceptée |
| [024](#adr-024--codes-deffacement-reed-solomon) | Codes d'effacement Reed-Solomon | Acceptée |
| [025](#adr-025--pas-de-scripts-côté-serveur) | Pas de scripts côté serveur | Acceptée |
| [026](#adr-026--transactions-soumises-en-une-fois) | Transactions soumises en une fois | Acceptée |
| [027](#adr-027--parité-de-performance-avec-les-services-remplacés) | Parité de performance avec les services remplacés | Acceptée |
| [028](#adr-028--cicd-sur-github-actions) | CI/CD sur GitHub Actions | Acceptée |
| [029](#adr-029--internationalisation-limitée-aux-interfaces-humaines) | Internationalisation limitée aux interfaces humaines | Acceptée |
| [030](#adr-030--arrêt-du-processus-en-cas-de-panique) | Arrêt du processus en cas de panique | Acceptée |
| [031](#adr-031--console-web-embarquée-en-svelte-et-typescript) | Console web embarquée en Svelte et TypeScript | Acceptée |

---

## ADR-001 — Licence Apache 2.0

- **Statut** : Acceptée
- **Exigences liées** : O6, C-02

### Contexte
Les changements de licence de Redis et le retrait de l'édition communautaire de MinIO sont une des motivations du projet. Les utilisateurs cherchent une licence permissive et stable.

### Options étudiées
1. AGPLv3 : protège contre la reprise propriétaire en service, mais freine l'adoption en entreprise.
2. MIT : très permissive, sans clause de brevets.
3. Apache 2.0 : permissive, avec octroi explicite de licence de brevets.

### Décision
Apache 2.0, avec un accord de contribution léger (DCO, *Developer Certificate of Origin*) plutôt qu'une cession de droits, pour rendre un futur changement de licence unilatéral impossible.

### Conséquences
- Adoption facilitée, y compris dans des produits commerciaux.
- Dépendances limitées aux licences compatibles (contrôle `cargo deny`).

---

## ADR-002 — Rust comme unique langage du serveur

- **Statut** : Acceptée
- **Exigences liées** : C-01, ENF-SEC-01, ENF-PERF

### Contexte
Le moteur doit être sûr en mémoire, prévisible en latence (pas de ramasse-miettes) et performant.

### Décision
Le serveur est entièrement en Rust. Les SDK Python et TypeScript sont dans leurs langages ; le SDK Python PEUT utiliser le client Rust via des liaisons (`pyo3`) pour les fonctions intensives.

### Conséquences
- Pas de pauses de ramasse-miettes, sécurité mémoire hors blocs `unsafe` identifiés.
- Temps de compilation élevés : cache de compilation (`sccache`) en CI, découpage en crates.
- Recrutement de contributeurs plus restreint que pour Go.

---

## ADR-003 — Thread-per-core avec `tokio` en mode `current_thread`

- **Statut** : Acceptée
- **Exigences liées** : ENF-PERF-01/02, ENF-ISO-01/02

### Contexte
L'isolation entre charges et la latence du clé-valeur exigent que chaque cœur possède ses données sans verrous partagés.

### Options étudiées
1. `tokio` multi-thread avec vol de tâches : écosystème riche, mais tâches migrant entre cœurs, verrous partagés, latence moins prévisible.
2. `glommio` ou `monoio` (thread-per-core natif, `io_uring`) : modèle idéal, mais écosystème plus restreint (HTTP, gRPC, TLS) et Linux uniquement.
3. `tokio` en mode `current_thread`, un runtime par cœur épinglé : thread-per-core effectif, compatible avec `hyper`, `axum`, `tonic`, `rustls`, et portable.

### Décision
Option 3. Les E/S disque passent par l'ordonnanceur d'E/S de Ndaal (ADR-016), pas par `tokio::fs`. La messagerie inter-cœurs utilise des files SPSC dédiées.

### Conséquences
- Accès immédiat à l'écosystème `tokio`.
- `io_uring` intégré via notre propre backend d'E/S plutôt que via le runtime.
- Si les mesures montrent une limite due au runtime, une migration vers `monoio` reste possible derrière les abstractions de `ndaal-runtime` (ADR à rédiger le cas échéant).

---

## ADR-004 — Un journal unique multiplexé par shard

- **Statut** : Acceptée
- **Exigences liées** : EF-TX-01, ENF-PERF-03, ENF-DUR-01

### Contexte
Les trois modèles doivent partager un journal pour permettre l'atomicité multi-modèles. Avec des milliers de partitions, un fichier de journal par partition multiplierait les `fsync`.

### Options étudiées
1. Un journal par partition : simple, mais beaucoup de petits `fsync`.
2. Un journal par nœud : un seul flux, mais contention entre cœurs.
3. Un journal par shard (cœur), multiplexant les partitions du shard, avec un LSN logique par partition.

### Décision
Option 3. Le journal Raft de chaque partition est stocké dans ce même flux (pas de double journal).

### Conséquences
- Écriture groupée efficace, pas de contention entre cœurs.
- La troncature d'un fichier de journal dépend du point de contrôle de **toutes** les partitions qui y écrivent : une partition lente à faire son point de contrôle retient le journal. Atténuation : point de contrôle forcé d'une partition qui retient plus de N fichiers.
- La migration d'une partition vers un autre shard implique un point de contrôle préalable.

---

## ADR-005 — L'espace de noms comme unité de transaction et de placement

- **Statut** : Acceptée
- **Exigences liées** : EF-GEN-02, EF-TX-01/02, EF-DIST-03

### Contexte
Les transactions distribuées sont coûteuses et complexes. Les cas d'usage visés regroupent naturellement les données par application ou par locataire.

### Décision
- Mode `single` (défaut) : un espace de noms entier dans une partition ; transactions sur tout l'espace.
- Mode `sharded` : réparti par clé de routage (étiquettes `{…}` à la Redis Cluster) ; transactions au sein d'un groupe de cohérence.

### Conséquences
- Transactions toujours locales à un shard : sérialisables sans verrou ni coordination.
- Débit d'écriture d'un espace `single` borné par un cœur : documenté, avec passage à `sharded` par division en ligne.
- Les développeurs doivent concevoir leurs clés de routage en espace réparti (documentation et exemples nécessaires).

---

## ADR-006 — Pas de transactions entre espaces de noms

- **Statut** : Acceptée
- **Exigences liées** : périmètre §4.2

### Décision
Les transactions entre espaces de noms ne sont pas prévues. Les transactions entre groupes de cohérence d'un même espace réparti sont une extension souhaitable (validation en deux phases), étudiée après les fonctions essentielles.

### Conséquences
Code de transaction simple et rapide ; limitation documentée.

---

## ADR-007 — Journalisation logique idempotente par version

- **Statut** : Acceptée
- **Exigences liées** : ENF-DUR-01, EF-KV-09

### Contexte
Les commandes Redis comme `INCR`, `SPOP` ou `LREM` ne sont pas idempotentes ou dépendent de l'aléa. Les points de contrôle doivent pouvoir se faire sans arrêter les écritures.

### Options étudiées
1. Journaliser les commandes brutes et faire des points de contrôle cohérents (gel des écritures ou copie à l'écriture par `fork`) : non idempotent, pauses ou doublement de la mémoire.
2. Journaliser les effets (résultats) avec une version par élément et rejouer conditionnellement.

### Décision
Option 2 : chaque effet porte la version (LSN) obtenue ; à la reprise, un effet n'est appliqué que si la version de l'élément est inférieure.

### Conséquences
- Points de contrôle « flous » sans pause ni `fork`.
- Les effets peuvent être plus gros que les commandes (par exemple `SET` de la nouvelle valeur après `APPEND` sur une grande chaîne). Atténuation : effets différentiels déterministes pour les structures volumineuses (`LIST_PUSH`, `HASH_SET`, et `APPEND` journalisé comme ajout avec longueur attendue).
- Coût mémoire de la version par clé (compensé par un encodage compact).

---

## ADR-008 — Raft par partition avec `openraft`

- **Statut** : Acceptée
- **Exigences liées** : EF-DIST-01/02, ENF-DUR-02

### Options étudiées
1. Écrire notre propre Raft : contrôle total, risque élevé.
2. `raft-rs` (TiKV) : éprouvé, mais API de bas niveau et intégration du stockage plus laborieuse.
3. `openraft` : asynchrone, stockage et réseau injectables, utilisé en production par plusieurs projets.

### Décision
`openraft`, avec notre stockage de journal (ADR-004) et notre protocole réseau (ADR-014). Un groupe Raft par partition, avec regroupement des battements de cœur et des messages par paire de nœuds.

### Conséquences
- Dépendance critique : version figée, tests de simulation déterministe couvrant notre intégration ; possibilité de fork si nécessaire.
- Le regroupement des messages multi-groupes est à notre charge.

---

## ADR-009 — Stockage objet comme fondation optionnelle

- **Statut** : Acceptée
- **Exigences liées** : EF-NIV-01/02

### Décision
Deux modes : `local` (défaut en nœud unique, NVMe = stockage principal) et `disaggregated` (stockage objet = source durable, NVMe = journal + cache). Le passage de `local` à `disaggregated` est possible en ligne (envoi progressif des segments).

### Conséquences
- Le petit déploiement ne dépend d'aucun service externe.
- Le grand déploiement profite du coût et de l'élasticité du stockage objet, avec reconstruction rapide des nœuds.
- Deux chemins de configuration à tester.

---

## ADR-010 — Octets des objets hors du journal

- **Statut** : Acceptée
- **Exigences liées** : EF-OBJ-04, EF-TX-04, ENF-PERF-07

### Décision
Seules les métadonnées des objets passent par le journal (et par Raft). Les octets sont écrits directement dans le stockage de blocs avant la validation. Les petits objets (≤ 64 Kio) sont en ligne dans le journal.

### Conséquences
- Journal petit et rapide ; transactions incluant des objets très volumineux.
- Nécessité d'un ramasse-miettes des blocs orphelins (ADR-022) et d'une réplication propre aux blocs.

---

## ADR-011 — Implémentation HNSW propre en Rust

- **Statut** : Acceptée
- **Exigences liées** : EF-VEC-05/06/08, C-01

### Options étudiées
1. `usearch` (C++ avec liaisons) : très performant, mais dépendance C++ (contraire à C-01) et intégration du format de segment et du filtrage moins maîtrisée.
2. `hnsw_rs` (Rust) : utilisable pour un prototype ; format et filtrage non adaptés à nos segments.
3. Implémentation propre.

### Décision
Implémentation propre dans `ndaal-vector`, avec `hnsw_rs` comme référence de rappel dans les tests. L'index fait partie de la valeur du produit (filtrage intégré, liens par valeur, format de segment, `mmap`).

### Conséquences
Effort significatif ; bancs de rappel obligatoires (jeux publics SIFT, GloVe, et un jeu de dimension 768).

---

## ADR-012 — Index disque de type Vamana/DiskANN

- **Statut** : Acceptée
- **Exigences liées** : EF-VEC-13, ENF-PERF-05

### Décision
Graphe Vamana avec vecteurs PQ en RAM et blocs de 4 Kio par nœud sur NVMe ; recherche en faisceau avec lectures parallèles.

### Conséquences
Permet des collections bien plus grandes que la RAM ; latence dépendante du NVMe ; construction plus coûteuse (faite sur cœurs `vector` en priorité basse).

---

## ADR-013 — Couche S3 basée sur `s3s`

- **Statut** : Acceptée
- **Exigences liées** : EF-OBJ, ENF-COMPAT-02/03

### Contexte
L'API S3 est vaste (XML, en-têtes, SigV4, cas limites). La crate `s3s` (Apache-2.0) fournit les types, le routage et la vérification des signatures.

### Décision
Utiliser `s3s` pour la couche protocolaire ; notre code implémente son interface de service. Si `s3s` devenait bloquant, le remplacement est circonscrit à `ndaal-proto-s3`.

### Conséquences
Gain de temps important ; dépendance à suivre (`cargo deny`, épinglage de version).

---

## ADR-014 — Protocole interne binaire léger

- **Statut** : Acceptée
- **Exigences liées** : ENF-PERF-03, EF-DIST

### Options étudiées
1. gRPC (`tonic`) : outillage, mais surcoût HTTP/2 et protobuf sur le chemin critique de la réplication.
2. Trames binaires propres sur TCP + mTLS, sérialisation `postcard`.

### Décision
Option 2 pour le plan de données (Raft, routage, blocs) ; gRPC reste disponible pour l'API publique.

### Conséquences
Moins de surcoût, contrôle total du multiplexage et de la priorité ; versionnement du protocole à notre charge (numéro de protocole négocié à la connexion).

---

## ADR-015 — Protocol Buffers pour les métadonnées évolutives

- **Statut** : Acceptée
- **Exigences liées** : ENF-QUAL-03, ENF-COMPAT-04

### Décision
Formats chauds (journal, blocs de données des segments) : binaire propre optimisé. Structures évolutives (manifestes, catalogue, métadonnées objet, répertoires de sections) : protobuf via `prost` (Rust pur).

### Conséquences
Évolution de schéma facile (champs optionnels), coût négligeable sur ces structures peu fréquentes.

---

## ADR-016 — Abstraction des E/S avec deux implémentations

- **Statut** : Acceptée
- **Exigences liées** : ENF-ISO-01, ENF-PORT-02

### Décision
Trait `IoBackend` dans `ndaal-io` avec :
1. un pool de threads d'E/S (`pread`/`pwrite`), portable, implémenté en premier ;
2. un backend `io_uring` (Linux), activé automatiquement si disponible.

Une troisième implémentation, simulée, sert à la simulation déterministe (ADR-017).

### Conséquences
Portabilité (macOS, Windows en développement) et performances maximales sous Linux ; les deux backends réels sont testés par la même suite.

---

## ADR-017 — Simulation déterministe

- **Statut** : Acceptée
- **Exigences liées** : ENF-DUR-01/02, ENF-QUAL-05

### Contexte
Les bogues de durabilité et de consensus apparaissent dans des entrelacements rares de pannes. Les tests classiques les trouvent mal.

### Options étudiées
1. `madsim` : simulation déterministe en remplaçant `tokio` à la compilation.
2. `turmoil` : simulation réseau pour `tokio`.
3. Abstractions propres (`Clock`, `Rng`, `Disk`, `Network`) avec implémentation simulée et exécution mono-thread.

### Décision
Option 3, éventuellement complétée par `turmoil` pour le réseau. Toute la logique du moteur est écrite contre les abstractions de `ndaal-io`. Le simulateur (`ndaal-sim`) exécute un cluster complet dans un seul thread, avec une graine aléatoire qui détermine tous les événements : latences, pannes de disque, écritures interrompues, partitions réseau, redémarrages, dérive d'horloge.

### Conséquences
- Tout échec est **reproductible** à partir de sa graine.
- Discipline stricte : aucune lecture directe d'horloge, de réseau, de disque ou d'aléa (vérifiée par lint maison : interdiction de `std::time::Instant::now`, `std::fs`, `rand::thread_rng` hors `ndaal-io`).

---

## ADR-018 — Compression LZ4 par défaut

- **Statut** : Acceptée
- **Exigences liées** : C-01, ENF-PERF

### Décision
`lz4_flex` (Rust pur) par défaut pour les segments et le journal ; Zstandard optionnel (fonction de compilation `zstd`, dépendance C) pour le niveau objet froid, où le taux de compression compte plus que la vitesse.

---

## ADR-019 — Allocateur mémoire

- **Statut** : Acceptée
- **Exigences liées** : ENF-RES-04/05

### Contexte
Les charges clé-valeur sont sensibles à la fragmentation. L'allocateur de `musl` est lent ; `jemalloc` et `mimalloc` sont en C.

### Décision
- Binaire Linux de production : `mimalloc` (fonction de compilation activée par défaut, exception documentée à C-01 car hors logique métier et remplaçable).
- Repli : allocateur système (fonction désactivable).
- Les grands tampons (segments, tables) utilisent des allocations dédiées alignées, comptabilisées (§4.5 de l'architecture).
- Révision prévue si un allocateur Rust pur atteint des performances comparables.

---

## ADR-020 — Exception cryptographique `ring`

- **Statut** : Acceptée
- **Exigences liées** : C-01, EF-SEC-01/05

### Décision
`rustls` avec le fournisseur cryptographique `ring` (qui contient des primitives en assembleur et en C auditées). Les primitives applicatives (AES-GCM, SHA-2, HMAC) utilisent aussi `ring` ou les crates RustCrypto (`aes-gcm`, `sha2`, `md-5`, `hmac`).

### Conséquences
Exception unique et documentée à C-01 ; justification : une cryptographie éprouvée et rapide prime sur la pureté du langage.

---

## ADR-021 — Cache hybride `foyer`

- **Statut** : Acceptée
- **Exigences liées** : EF-NIV-03/06

### Décision
Utiliser `foyer` (cache hybride mémoire + disque en Rust, Apache-2.0) pour le cache de blocs de segments, de blocs d'objets et de segments distants, avec admission TinyLFU. Nos propres structures (tables clé-valeur, graphes HNSW) restent gérées par leurs moteurs ; seul le niveau « blocs » passe par `foyer`.

### Conséquences
Gain de temps important ; dépendance suivie ; métriques de `foyer` exportées.

---

## ADR-022 — Ramasse-miettes des blocs par marquage-balayage

- **Statut** : Acceptée
- **Exigences liées** : EF-TX-04, EF-OBJ-04

### Options étudiées
1. Compteurs de références journalisés : précis, mais chaque copie, suppression ou réessai doit les maintenir correctement, et une erreur entraîne une perte de données (suppression d'un bloc encore utilisé).
2. Marquage-balayage périodique avec marge de sécurité temporelle.

### Décision
Option 2. Un bloc n'est supprimé que s'il n'est référencé par aucune métadonnée à un LSN de marquage **et** qu'il est plus ancien que la marge (24 h par défaut).

### Conséquences
Robustesse (une erreur retarde la récupération d'espace au lieu de perdre des données) ; espace récupéré avec retard ; coût de marquage proportionnel au nombre de blocs (filtres compacts triés).

---

## ADR-023 — Plan de contrôle dans un groupe Raft système

- **Statut** : Acceptée
- **Exigences liées** : C-03, EF-DIST-03/05

### Décision
Le catalogue et la carte des partitions vivent dans un groupe Raft système interne (3 ou 5 nœuds `control`), sans service externe (pas d'etcd ni de ZooKeeper). Chaque nœud en garde une copie locale abonnée.

### Conséquences
Déploiement autonome ; le pilote de placement est un composant critique à tester en simulation ; sa charge est faible (pas sur le chemin des données).

---

## ADR-024 — Codes d'effacement Reed-Solomon

- **Statut** : Acceptée
- **Exigences liées** : EF-DIST-07, ENF-DUR-05

### Décision
Crate `reed-solomon-simd` (Rust pur, SIMD). Schémas par défaut : 4+2 jusqu'à 11 nœuds `storage`, 8+3 au-delà. Les petits blocs (< 1 Mio) sont **répliqués** (3 copies) plutôt qu'encodés, le surcoût de l'encodage n'étant pas rentable.

---

## ADR-025 — Pas de scripts côté serveur

- **Statut** : Acceptée
- **Exigences liées** : périmètre §4.2

### Décision
Pas de Lua (`EVAL`) ni de Functions Redis. Les besoins couverts par les scripts (opérations atomiques composées) le sont par les transactions avec préconditions. Une extension WebAssembly pourra être étudiée plus tard.

### Conséquences
Certaines bibliothèques Redis qui dépendent de scripts (verrous distribués de type Redlock, certaines files de tâches) ne fonctionneront pas : liste publiée dans la documentation de compatibilité, avec des alternatives (`SET NX PX` pour les verrous simples, Streams pour les files).

---

## ADR-026 — Transactions soumises en une fois

- **Statut** : Acceptée
- **Exigences liées** : EF-TX-01/02/03

### Contexte
Les transactions interactives (ouvrir, lire, écrire, valider avec allers-retours client) obligent à garder des verrous ou des versions pendant la durée réseau du client.

### Décision
Une transaction est soumise en une seule requête : préconditions + opérations. La lecture-puis-écriture se fait par concurrence optimiste : le client lit (avec versions), puis soumet une transaction avec préconditions de version, et réessaie en cas d'échec.

### Conséquences
Shard jamais bloqué par un client lent ; modèle familier (`WATCH`/`MULTI`/`EXEC`) ; le SDK fournit une aide « lire-modifier-écrire avec réessais ».

---

## ADR-027 — Parité de performance avec les services remplacés

- **Statut** : Acceptée
- **Date** : 2026-10-12
- **Exigences liées** : O7, ENF-COMP-01 à 08, ENF-PERF, R-03

### Contexte
Ndaal remplace Redis/Valkey, Qdrant et MinIO. Un utilisateur n'adoptera pas un système unifié s'il doit accepter des performances moindres : la simplicité d'exploitation et les transactions multi-modèles ne compensent pas une latence de cache ou un débit de recherche inférieurs. Des objectifs exprimés seulement en valeurs absolues (ENF-PERF) peuvent être atteints tout en restant derrière les services remplacés.

### Options étudiées
1. **Objectifs absolus seuls** : simples à mesurer, mais ne garantissent rien face aux concurrents, dont les performances évoluent.
2. **Parité obligatoire, dépassement visé** : chaque banc important est exécuté côte à côte avec la référence ; être derrière est un défaut bloquant.
3. **Dépassement obligatoire partout** : ambitieux, mais irréaliste sur certains cas où la référence est déjà proche des limites du matériel (débit séquentiel des disques, par exemple).

### Décision
Option 2. Ndaal DOIT atteindre au moins 95 % du débit et au plus 105 % de la latence p99 de la référence sur chaque banc ENF-COMP (tolérance de bruit de mesure), et DEVRAIT la dépasser. Sur la charge mixte RAG (ENF-COMP-08), Ndaal DOIT être meilleur que les trois services réunis, puisqu'il supprime les allers-retours entre eux.

Les comparaisons sont équitables : même machine, même budget CPU, même client, **durabilité équivalente**, référence réglée selon les recommandations de son éditeur, comparaison vectorielle à rappel égal (voir [STRATEGIE_DE_TESTS §9.4](STRATEGIE_DE_TESTS.md#94-bancs-comparatifs-parité-avec-les-services-remplacés)).

### Conséquences
- L'infrastructure de bancs comparatifs est construite dès J01 (T01.18) ; chaque jalon de modèle (J03, J04, J05, J06) mesure son écart, J07 le résorbe et ne peut pas être clos sans parité.
- Toute fonctionnalité qui coûte en performance sur le chemin court (nouvelle vérification, nouvel effet journalisé, nouvelle indirection) doit être mesurée contre la référence avant fusion.
- Les résultats sont publiés tels quels, y compris lorsqu'ils sont défavorables.
- Les versions des références évoluent : un recul relatif de Ndaal après une nouvelle version de Valkey, Qdrant ou MinIO est traité comme une régression.

---

## ADR-028 — CI/CD sur GitHub Actions

- **Statut** : Acceptée
- **Date** : 2026-10-12
- **Exigences liées** : ENF-QUAL-06, ENF-QUAL-07, EF-OPS-13, ENF-PORT-02

### Contexte
La stratégie de tests impose des vérifications à chaque demande de fusion, chaque nuit et avant chaque publication, sur Linux, macOS et Windows, ainsi qu'une publication signée et automatisée. Il faut un système de CI/CD unique.

### Options étudiées
1. **GitHub Actions** : intégré à l'hébergement du dépôt, exécuteurs Linux (x86_64 et arm64), macOS et Windows fournis, exécuteurs auto-hébergés possibles, jetons OIDC pour la signature sans clé.
2. **GitLab CI** : complet, mais impose d'héberger le dépôt sur GitLab ou de le dupliquer.
3. **Buildkite ou Jenkins** : souples, mais toute l'infrastructure est à notre charge.

### Décision
GitHub Actions. Les workflows sont décrits dans [STRATEGIE_DE_TESTS §13.4](STRATEGIE_DE_TESTS.md#134-mise-en-œuvre-sur-github-actions). Les bancs de performance tournent sur des **exécuteurs auto-hébergés** dédiés à l'environnement de référence, car les exécuteurs partagés sont trop bruités pour ENF-COMP.

### Conséquences
- Chaque vérification est un script du dépôt (`scripts/`), appelé par le workflow : la CI reste reproductible en local et migrable vers un autre système.
- Actions tierces épinglées par empreinte de commit, permissions minimales, aucun secret exposé aux demandes de fusion venant de forks.
- Coût et maintenance des exécuteurs auto-hébergés à prévoir.

---

## ADR-029 — Internationalisation limitée aux interfaces humaines

- **Statut** : Acceptée
- **Date** : 2026-10-12
- **Exigences liées** : ENF-I18N-01 à 06, ENF-COMPAT-01, ENF-COMPAT-02

### Contexte
Les utilisateurs de Ndaal ne parlent pas tous anglais ; la console web et la CLI gagnent à être traduites. Mais les clients Redis analysent les messages d'erreur (`ERR`, `WRONGTYPE`), les SDK S3 analysent les codes (`NoSuchKey`), et les outils de supervision recherchent des textes précis dans les journaux.

### Options étudiées
1. **Tout traduire**, y compris protocoles et journaux : casse la compatibilité avec les clients existants et les outils d'exploitation.
2. **Ne rien traduire** : simple, mais exclut les utilisateurs non anglophones de la console et de la CLI.
3. **Traduire seulement les interfaces humaines** (console, CLI, documentation), garder les interfaces machine en anglais.

### Décision
Option 3, avec **Fluent** (`fluent-bundle`, projet Mozilla, Rust pur, licence Apache-2.0/MIT) : il gère les pluriels et les accords, que de simples tables clé-valeur gèrent mal. Langues livrées : anglais (référence) et français. Les traductions vivent dans `locales/<langue>/*.ftl`, chargées par la crate `ndaal-i18n`.

### Conséquences
- Toute chaîne destinée à un humain dans la CLI ou la console passe par une clé de traduction ; la CI vérifie la complétude.
- Les messages d'erreur des protocoles, les journaux et les métriques restent en anglais, quelle que soit la langue choisie.
- Ajouter une langue revient à ajouter un dossier de fichiers `.ftl`, sans modifier le code.

---

## ADR-030 — Arrêt du processus en cas de panique

- **Statut** : Acceptée
- **Date** : 2026-10-12
- **Exigences liées** : ENF-DUR-01, ENF-DUR-02, ENF-DUR-03

### Contexte
Dans un modèle thread-per-core, une panique dans le thread d'un cœur laisse les partitions de ce cœur dans un état inconnu. Tenter de continuer risque de servir ou d'écrire des données incohérentes.

### Options étudiées
1. **Rattraper la panique** (`catch_unwind`) et continuer : l'état en mémoire peut être corrompu, et la corruption peut atteindre le disque et les réplicas.
2. **Redémarrer seulement le cœur fautif** : complexe ; les messages en cours entre cœurs et l'état partagé du runtime restent douteux.
3. **Arrêter le processus** (`panic = "abort"`) et repartir du journal : conception *crash-only*.

### Décision
Option 3. Les binaires sont compilés avec `panic = "abort"`. Un gestionnaire de panique enregistre le contexte (message, pile d'appels, partition, identifiant de requête), puis le processus s'arrête. La reprise utilise le seul chemin de reprise existant : relecture du journal, ou basculement vers un réplica en cluster.

### Conséquences
- Un seul chemin de reprise à rendre fiable, testé en permanence par la simulation déterministe et les campagnes de pannes.
- Une panique coûte un redémarrage du nœud (ENF-PERF-09) : d'où l'importance de ne jamais paniquer sur une entrée externe (validation systématique, fuzzing).
- Les tests continuent d'utiliser le déroulement de pile (*unwind*), pour que `#[should_panic]` et les rapports de test fonctionnent.

---

## ADR-031 — Console web embarquée en Svelte et TypeScript

- **Statut** : Acceptée
- **Date** : 2026-10-12
- **Exigences liées** : EF-OPS-11, EF-OPS-14, ENF-UI-01 à 07, EF-GEN-01, C-01

### Contexte
Qdrant et MinIO embarquent une console web servie par le serveur ; Redis s'appuie sur RedisInsight, une application séparée. Un utilisateur qui passe à Ndaal ne doit rien perdre (O7) : il faut une console au moins équivalente, et elle peut montrer ce qu'aucun concurrent ne peut offrir, les liens entre objets, vecteurs et clés.

### Options étudiées
1. **Console minimale de supervision** : peu coûteuse, mais en recul par rapport aux consoles des services remplacés.
2. **Application séparée** (comme RedisInsight) : découplée, mais un logiciel de plus à installer, contraire à l'idée d'un binaire unique.
3. **Console embarquée en Rust compilé en WebAssembly** (Leptos) : un seul langage, mais un écosystème d'interface encore jeune (tableaux, graphiques, envoi de fichiers, éditeur de code) et peu de contributeurs ; effort estimé environ 30 % plus élevé.
4. **Console embarquée en Svelte + TypeScript** : écosystème mûr, fichiers produits légers, peu de dépendances, contributeurs nombreux ; introduit Node.js dans la construction.

### Décision
Option 4. La console est servie par chaque nœud sur `/console`, ses fichiers sont **intégrés au binaire** (contrairement à Qdrant, qui les livre à côté), et elle n'utilise que l'API HTTP publique.

La contrainte C-01 (« tout en Rust ») vise la performance et la sûreté du serveur ; elle ne s'applique pas au code exécuté dans le navigateur, qui n'est jamais nécessaire au fonctionnement du serveur.

Svelte est préféré à React pour la taille des fichiers produits et le nombre réduit de dépendances, donc une surface d'attaque plus faible côté npm.

### Conséquences
- Node.js est nécessaire pour construire la console, pas le serveur : la fonctionnalité Cargo `console` permet de construire le binaire sans Node.js. La CI construit toujours la version complète.
- Dépendances npm minimales, verrouillées et auditées en CI, au même titre que `cargo deny` pour Rust.
- Les traductions Fluent sont partagées entre la CLI et la console.
- Tests de bout en bout dans un navigateur (Playwright, trois moteurs) et contrôles d'accessibilité (axe) en CI.
- Toute nouvelle fonction de la console commence par un point d'accès public dans l'API : la console ne peut pas devenir une porte dérobée.
