# J01 lot 3 — `ndaal-common` et `ndaal-io` (disque, horloge, aléa) : plan d'implémentation

> **Pour les agents :** sous-compétence requise : superpowers:executing-plans (ou subagent-driven-development), tâche par tâche.
>
> **Pas d'étape de commit** : le responsable commite sur une branche `j01-lot3-common-io` et ouvre une demande de fusion ; la CI fait foi.

**Objectif :** les premières briques de code testables de Ndaal : identifiants, encodages et classification des erreurs (`ndaal-common`), et les abstractions d'E/S disque, horloge et aléa avec une implémentation réelle et une implémentation simulée capable de perdre exactement les écritures non synchronisées lors d'une coupure (`ndaal-io`).

**Architecture :** `ndaal-common` ne dépend que de `thiserror`. `ndaal-io` expose des traits (`Clock`, `Rng`, `Disk`, `File`) à méthodes `async` (répartition statique, sans `Send` imposé : modèle thread-per-core, ADR-003). Implémentation réelle : `std::fs` exécuté sur le pool bloquant de `tokio` (ADR-016, implémentation « pool de threads »). Implémentation simulée : machine à états déterministe pilotée par une graine (ADR-017), qui modélise le cache de pages.

**Pile :** Rust 1.99.0 ; `thiserror` 2.0.21, `tokio` 1.53.2 (fonctionnalité `rt`), `getrandom` 0.4.3 ; tests : `proptest` 1.12.0, `tempfile` 3.27.0.

**Spécification :** [CAHIER_DES_TACHES.md](../../../CAHIER_DES_TACHES.md#j01--fondations-du-projet) T01.05, T01.07, T01.08 ; [FORMATS_DE_DONNEES.md](../../../FORMATS_DE_DONNEES.md) §1 (conventions) et §10 (identifiants) ; [STRATEGIE_DE_TESTS.md §4](../../../STRATEGIE_DE_TESTS.md#4-simulation-déterministe) ; [DECISIONS.md](../../../DECISIONS.md) ADR-003, ADR-016, ADR-017 ; [ARCHITECTURE.md §18](../../../ARCHITECTURE.md#18-surcharge-et-gestion-des-erreurs) ; [CLAUDE.md](../../../CLAUDE.md) §3.2, §4.2, §4.8, §5.

## Contraintes globales

- Entiers de taille fixe en petit-boutiste ; `varint` = LEB128 non signé, au plus 10 octets pour un `u64` ; `zigzag` pour les signés (FORMATS §1).
- Horodatages : `u64` microsecondes depuis l'époque Unix (FORMATS §1).
- Aucun accès direct à l'horloge, au disque ou à l'aléa hors de `ndaal-io` (ADR-017) ; `ndaal-common` n'en fait aucun.
- Erreurs typées avec `thiserror`, classées `Retryable` / `Client` / `Fatal` (ARCHITECTURE §18) ; pas de `unwrap()` hors tests.
- Aucun décodeur ne panique sur une entrée invalide : il renvoie une erreur (CLAUDE.md §2).
- Test d'abord (CLAUDE.md §5) ; tests de propriétés aller-retour pour chaque encodage (critère de fin de T01.05).
- Pas de `unsafe` (même dans `ndaal-io`, qui en a le droit : rien ici n'en a besoin).

## Décisions de périmètre (à valider avec le plan)

1. **Le réseau passe au lot 4, avec l'ordonnanceur de simulation (T01.09).** Un réseau simulé utile livre les messages selon le temps virtuel et réveille les tâches en attente : il a besoin de l'ordonnanceur. T01.07 et T01.08 restent donc « en cours » après ce lot (reste : `Network`).
2. **Licence `Unicode-3.0` ajoutée à la liste autorisée (C-02).** `thiserror` dépend (via `proc-macro2`) de `unicode-ident`, sous `(MIT OR Apache-2.0) AND Unicode-3.0`. Unicode-3.0 est la licence qui a remplacé Unicode-DFS-2016, déjà autorisée ; elle est aussi permissive. Sans elle, presque aucune crate Rust ne passe `cargo deny`. Modification de C-02 dans le cahier des charges et de `deny.toml`.
3. **`Rng` sert l'aléa non cryptographique** (gigue, échantillonnage, choix de la simulation) : générateur xoshiro256++ amorcé par l'OS (`getrandom`) en réel, par la graine en simulation. L'aléa cryptographique (jetons, clés) passera par `ring` (ADR-020), jamais par `Rng`.
4. **`tokio` entre comme dépendance de `ndaal-io`** (fonctionnalité `rt`), conformément à ADR-003 : l'implémentation réelle du disque exécute `std::fs` sur son pool bloquant.
5. **`PointId` (identifiant de point vectoriel fourni par le client) est reporté à J05**, avec l'API vectorielle qui le définit.
6. **Les nombres magiques `STRWAL01`, `STRSEG01`, `STRBLK01`, `STRFRG01` deviennent `NDLWAL01`, `NDLSEG01`, `NDLBLK01`, `NDLFRG01`** dans FORMATS_DE_DONNEES.md : vestige du nom provisoire « Strata », aucun code ne les utilise encore.
7. **Exception à la règle petit-boutiste (FORMATS §1) pour les identifiants composés de 16 octets** (`SegmentId`, `ChunkId`, `VersionId`) : ils sont encodés en **gros-boutiste**, pour que l'ordre de leurs octets (noms de fichiers, clés triées, chaîne base32) suive l'ordre de leurs valeurs. La règle petit-boutiste reste valable pour les champs numériques des formats. Ajout d'une phrase en ce sens dans FORMATS §1 et §10 (tâche 6).
8. **`DecodeError` est classée `Client`** : le décodeur ne sait pas si l'octet vient d'un client ou du disque ; le code qui décode une donnée lue sur disque la convertit en erreur `Fatal` (corruption) dans son propre type.

## Points à surveiller en revue

1. **`varint` hostile** : 11 octets, octets de continuation sans fin, valeur qui dépasse `u64`, encodage non minimal (`0x80 0x00`) : erreur, jamais de panique ni de lecture hors limites. Testé à la tâche 1.
2. **Chaîne tronquée** (longueur annoncée plus grande que les octets restants, ou proche de `u64::MAX`) : erreur sans allocation géante. Testé à la tâche 1.
3. **Coupure pendant un renommage ou une création de fichier non suivis de `sync_dir`** : le fichier peut disparaître ou garder son ancien nom, comme sur un vrai système de fichiers. Testé à la tâche 5.
4. **Lecture au-delà de la fin du fichier, écriture creuse loin après la fin** : lecture courte (pas d'erreur), trou rempli de zéros. Testé aux tâches 4 et 5.
5. **Chemins Windows** (séparateurs, `sync_dir` sans équivalent) : l'implémentation réelle passe sur les 4 systèmes de la CI. Testé à la tâche 4.

---

### Tâche 1 : encodages d'octets (`ndaal-common::bytes`)

**Fichiers :** Créer `crates/ndaal-common/src/bytes.rs` ; Modifier `crates/ndaal-common/src/lib.rs`, `crates/ndaal-common/Cargo.toml`, `Cargo.toml` (dépendances de l'espace de travail).

**Interfaces (produit) :**
- `pub enum DecodeError { UnexpectedEnd { needed: usize, remaining: usize }, VarintTooLong, VarintOverflow, VarintNotMinimal, InvalidUtf8, InvalidValue(&'static str) }` (`thiserror`).
- `pub fn put_varint(out: &mut Vec<u8>, v: u64)` ; `pub fn get_varint(input: &mut &[u8]) -> Result<u64, DecodeError>` (avance la tranche).
- `pub fn zigzag_encode(v: i64) -> u64` ; `pub fn zigzag_decode(v: u64) -> i64`.
- `pub fn put_bytes(out: &mut Vec<u8>, b: &[u8])` ; `pub fn get_bytes<'a>(input: &mut &'a [u8]) -> Result<&'a [u8], DecodeError>` (longueur `varint` + octets, sans copie).
- `pub fn put_str(out, s: &str)` ; `pub fn get_str<'a>(input) -> Result<&'a str, DecodeError>`.
- `pub fn put_u16_le / put_u32_le / put_u64_le` et `get_u16_le / get_u32_le / get_u64_le(input: &mut &[u8]) -> Result<_, DecodeError>`.

- [ ] **Étape 1 : tests qui échouent** (`#[cfg(test)]` dans `bytes.rs`) :
  - propriétés (`proptest`) : aller-retour `varint` pour tout `u64` ; `zigzag` pour tout `i64` ; `bytes`, `str`, `u16/u32/u64` ; une suite de valeurs concaténées se relit dans l'ordre et laisse une tranche vide ;
  - valeurs fixées : `put_varint(0) == [0x00]`, `put_varint(127) == [0x7F]`, `put_varint(128) == [0x80, 0x01]`, `put_varint(300) == [0xAC, 0x02]`, `put_varint(u64::MAX)` fait 10 octets ; `zigzag_encode(0, -1, 1, -2) == (0, 1, 2, 3)` ;
  - erreurs : `[]` → `UnexpectedEnd` ; `[0x80; 11]` → `VarintTooLong` ; `[0xFF ×9, 0x7F]` → `VarintOverflow` ; `[0x80, 0x00]` → `VarintNotMinimal` ; longueur `varint` `u64::MAX` suivie de 3 octets → `UnexpectedEnd` sans allocation ; `get_str` sur `[0x01, 0xFF]` → `InvalidUtf8` ; en cas d'erreur, la tranche d'entrée n'est pas avancée.
- [ ] **Étape 2 :** `cargo nextest run -p ndaal-common` → échec (fonctions absentes).
- [ ] **Étape 3 :** implémenter.
- [ ] **Étape 4 :** `cargo nextest run -p ndaal-common` → tout passe.

---

### Tâche 2 : identifiants (`ndaal-common::ids`)

**Fichiers :** Créer `crates/ndaal-common/src/ids.rs`, `crates/ndaal-common/src/base32.rs`.

**Interfaces (produit) :**
- `NamespaceId(u32)`, `PartitionId(u32)`, `CollectionId(u32)`, `BucketId(u32)` : `Copy`, `Eq`, `Ord`, `Hash`, `Debug`, `Display` (décimal).
- `Lsn(u64)` avec `Lsn::FIRST == Lsn(1)` et `fn next(self) -> Lsn` (dépassement : panique documentée, impossible en pratique à 2^64).
- `NodeId([u8; 16])`, `ClusterId([u8; 16])` : `fn from_random_bytes(b: [u8; 16]) -> Self` fixe les bits de version 4 et de variante RFC 4122 ; `Display` au format UUID `8-4-4-4-12` en minuscules ; `FromStr`.
- `SegmentId` et `ChunkId` : `{ partition: PartitionId, lsn: Lsn, counter: u32 }`, `to_bytes() -> [u8; 16]` dans l'ordre `partition (4) · lsn (8) · compteur (4)` en **gros-boutiste** (pour que l'ordre des octets suive l'ordre des valeurs) et `from_bytes`.
- `VersionId { timestamp_micros: u64, lsn: Lsn }` : 16 octets gros-boutistes, `Display`/`FromStr` en base32 Crockford, 26 caractères.
- `TxId(u128)` : `fn new(p: PartitionId, lsn: Lsn) -> TxId` (`partition` dans les bits 64 à 95, `lsn` dans les bits 0 à 63) et `fn partition()`, `fn lsn()`.
- `StagedId` (préfixe `stg_` + ULID 26 caractères) et `UploadId` (préfixe `mpu_` + 26 caractères base32) : construits à partir de leurs composants (`timestamp_ms: u64` sur 48 bits, `random: [u8; 10]`), `Display`/`FromStr`. La génération (horloge et aléa) appartient à l'appelant.
- `base32` : `pub fn encode_crockford(bytes: &[u8; 16]) -> String` (26 caractères, 128 bits complétés à 130 par 2 bits de tête nuls) et `pub fn decode_crockford(s: &str) -> Result<[u8; 16], DecodeError>` (insensible à la casse, `I`/`L` → 1, `O` → 0, refuse `U` et toute autre lettre, refuse une valeur qui dépasse 128 bits).

- [ ] **Étape 1 : tests qui échouent** : aller-retour (propriétés) pour chaque identifiant et pour `base32` ; ordre : pour deux `SegmentId` a < b (ordre des champs), `a.to_bytes() < b.to_bytes()` ; `VersionId` : ordre chronologique conservé par la chaîne base32 ; UUID : `from_random_bytes([0xFF; 16])` s'affiche `ffffffff-ffff-4fff-bfff-ffffffffffff` ; `FromStr` refuse une chaîne de mauvaise longueur, un caractère invalide, un préfixe absent (`stg_`, `mpu_`) ; ULID connu : horodatage 0 et aléa nul → `stg_00000000000000000000000000`.
- [ ] **Étape 2 :** constater l'échec. **Étape 3 :** implémenter. **Étape 4 :** tout passe.

---

### Tâche 3 : classification des erreurs (`ndaal-common::error`)

**Fichiers :** Créer `crates/ndaal-common/src/error.rs`.

**Interfaces (produit) :**
- `pub enum ErrorClass { Retryable, Client, Fatal }` (ARCHITECTURE §18).
- `pub trait Classify { fn class(&self) -> ErrorClass; }`
- `impl Classify for DecodeError` → `Client` (décision 8), documenté sur le type.
- `impl Classify for std::io::Error` : `Interrupted`, `WouldBlock`, `TimedOut` → `Retryable` ; `StorageFull`, `QuotaExceeded` → `Retryable` ; tout le reste → `Fatal`.

- [ ] **Étape 1 :** tests de la table ci-dessus. **Étapes 2 à 4 :** échec, implémentation, succès.

---

### Tâche 4 : `ndaal-io`, traits et implémentation réelle (T01.07, partie disque, horloge, aléa)

**Fichiers :** Créer `crates/ndaal-io/src/{clock.rs, rng.rs, disk.rs, real/mod.rs, real/clock.rs, real/rng.rs, real/disk.rs}`, `crates/ndaal-io/tests/real_disk.rs`.

**Interfaces (produit) :**
```rust
pub trait Clock {
    /// Monotonic time since an arbitrary origin; never goes backwards.
    fn monotonic(&self) -> std::time::Duration;
    /// Wall-clock time, microseconds since the Unix epoch (FORMATS section 1).
    fn unix_micros(&self) -> u64;
}
pub trait Rng {
    fn next_u64(&mut self) -> u64;
    fn fill_bytes(&mut self, out: &mut [u8]);
}
pub struct OpenOptions { pub create: bool, pub truncate: bool }
pub trait Disk {
    type File: File;
    async fn open(&self, path: &Path, options: OpenOptions) -> io::Result<Self::File>;
    async fn remove(&self, path: &Path) -> io::Result<()>;
    async fn rename(&self, from: &Path, to: &Path) -> io::Result<()>;
    async fn create_dir_all(&self, path: &Path) -> io::Result<()>;
    async fn list_dir(&self, path: &Path) -> io::Result<Vec<PathBuf>>; // sorted
    /// Makes create, rename and remove in `dir` durable.
    async fn sync_dir(&self, dir: &Path) -> io::Result<()>;
}
pub trait File {
    async fn read_at(&self, buf: &mut [u8], offset: u64) -> io::Result<usize>; // short read at end
    async fn write_all_at(&self, buf: &[u8], offset: u64) -> io::Result<()>;
    async fn sync_data(&self) -> io::Result<()>;
    async fn len(&self) -> io::Result<u64>;
    async fn set_len(&self, len: u64) -> io::Result<()>;
}
```
- `Xoshiro256PlusPlus` (public, dans `rng.rs`) : `from_seed(u64)` via SplitMix64, `impl Rng` ; vérifié contre les vecteurs de la publication de référence (graine SplitMix64 = 0 : les 4 premières sorties de SplitMix64 sont fixées dans le test).
- Réel : `SystemClock` (`Instant` + `SystemTime`), `OsRng` (xoshiro amorcé par `getrandom`), `RealDisk` / `RealFile` (`std::fs` + lecture/écriture positionnelles `std::os::unix::fs::FileExt` ou `std::os::windows::fs::FileExt`, exécutées par `tokio::task::spawn_blocking`). `sync_dir` : `File::open(dir)?.sync_all()` sous Unix, sans effet sous Windows (pas d'équivalent ; documenté).

- [ ] **Étape 1 : tests qui échouent** (`tests/real_disk.rs`, `#[tokio::test(flavor = "current_thread")]`, dossier `tempfile::TempDir`) : écrire puis relire ; lecture au-delà de la fin → lecture courte ; écriture creuse à 1 Mio d'un fichier vide → `len` = 1 Mio + taille, trou à zéro ; `sync_data` réussit ; `rename` puis `list_dir` triée ; `remove` ; `open` sans `create` d'un fichier absent → `NotFound` ; `set_len` tronque ; `sync_dir` réussit. Horloge : deux appels successifs de `monotonic` sont croissants ; `unix_micros` est après le 2026-01-01. `Xoshiro256PlusPlus` : vecteurs de référence ; deux `OsRng` donnent des suites différentes.
- [ ] **Étapes 2 à 4 :** échec, implémentation, succès ; puis `./scripts/check-linux.sh` (chemin `cfg(unix)` compilé et testé sous Linux).

---

### Tâche 5 : `ndaal-io`, simulation du disque, de l'horloge et de l'aléa (T01.08, partie disque, horloge, aléa)

**Fichiers :** Créer `crates/ndaal-io/src/sim/{mod.rs, clock.rs, disk.rs}`, `crates/ndaal-io/tests/sim_disk.rs`.

**Interfaces (produit) :**
- `SimClock` (clonable, état partagé `Rc<Cell<..>>`) : `new(start_unix_micros: u64)`, `advance(d: Duration)`, `impl Clock`.
- Aléa simulé : `Xoshiro256PlusPlus::from_seed(graine)` (pas de nouveau type).
- `SimDisk` (clonable, état partagé `Rc<RefCell<..>>`) : `new(seed: u64)`, `impl Disk` ; tous les `async fn` se terminent sans attendre.
- Modèle : chaque fichier a un contenu **durable** et une liste ordonnée d'**écritures en attente** (cache de pages) ; les lectures voient le durable plus les écritures en attente ; `sync_data` rend durables les écritures en attente du fichier ; les créations, renommages et suppressions sont en attente jusqu'au `sync_dir` du dossier parent.
- `crash(&self, mode: CrashMode)` — `CrashMode::LoseAllUnsynced` (tout ce qui n'est pas synchronisé disparaît) ou `CrashMode::Random` (graine : chaque écriture en attente est perdue, appliquée, ou appliquée partiellement par un préfixe aligné sur 512 octets ; chaque opération de dossier en attente est perdue ou appliquée) ; ensuite, les fichiers ouverts sont invalides (`io::ErrorKind::Other`, message « disk crashed »).
- Pannes injectables : `fail_next(path: &Path, kind: io::ErrorKind)` (la prochaine opération sur ce fichier échoue) ; `set_capacity(bytes: u64)` (au-delà : `StorageFull`) ; `corrupt(path: &Path, offset: u64)` (inverse un bit du contenu durable : corruption silencieuse).

- [ ] **Étape 1 : tests qui échouent** (`tests/sim_disk.rs`) :
  - **critère de fin T01.08** (propriété) : suite aléatoire d'écritures, de `sync_data` et de `sync_dir` sur quelques fichiers, puis `crash(LoseAllUnsynced)` → le contenu de chaque fichier est exactement celui d'un modèle qui n'applique que les opérations synchronisées ;
  - propriété `crash(Random)` : les octets synchronisés sont toujours intacts ; chaque autre octet vient soit de l'état synchronisé, soit d'une écriture en attente qui le couvrait ;
  - même graine → même résultat de `crash(Random)` (déterminisme) ;
  - fichier créé puis écrit et synchronisé, **sans** `sync_dir`, puis `crash(LoseAllUnsynced)` → le fichier n'existe plus (point de revue 3) ;
  - renommage non suivi de `sync_dir` → après coupure, l'ancien nom existe et le nouveau non ;
  - `fail_next` → l'opération échoue avec le bon `ErrorKind`, la suivante réussit ; `set_capacity` → `StorageFull` ; `corrupt` → un bit diffère à la relecture ;
  - lecture au-delà de la fin et écriture creuse : même comportement que le disque réel (tâche 4) ;
  - `SimClock` : `advance` fait avancer `monotonic` et `unix_micros` du même intervalle ; rien ne bouge sans `advance`.
- [ ] **Étapes 2 à 4 :** échec, implémentation, succès.

---

### Tâche 6 : documentation et suivi

**Fichiers :** Modifier `FORMATS_DE_DONNEES.md` (nombres magiques, décision 6), `CAHIER_DES_CHARGES.md` (C-02 : ajouter `Unicode-3.0`), `deny.toml` (même ajout), `CAHIER_DES_TACHES.md`, `POINTS_REPORTES.md`.

- [ ] **Étape 1 :** appliquer les décisions 2, 6 et 7 ; `./scripts/check-deny.sh` passe avec les nouvelles dépendances.
- [ ] **Étape 2 :** chaîne complète de CLAUDE.md §11.
- [ ] **Étape 3 :** suivi : cocher **T01.05** ; T01.07 et T01.08 « en cours, reste : `Network` (lot 4) » ; tableau de bord.
- [ ] **Étape 4 :** revue finale par un relecteur neuf ; commandes git (branche, demande de fusion).
