# T02.13 — Enveloppe de segment générique : plan d'implémentation

> **Pour les agents :** sous-compétence requise : superpowers:executing-plans, tâche par tâche.
>
> **Pas d'étape de commit** : le responsable commite sur la branche `t0213-segment`, puis ouvre la demande de fusion avec `gh pr create`. Le compte rendu donne les commandes git, et la description de la demande de fusion est écrite dans `.git/pr-body.md`.

**Objectif :** une enveloppe de segment (en-tête, sections de blocs avec CRC ensemencé et LZ4, répertoire protobuf, pied) qui détecte toute corruption et vérifie les numéros de format à l'ouverture.

**Architecture :** module `ndaal-storage::segment`. Un codec pur (`format.rs`, `block.rs`, `directory.rs`) sur des tranches d'octets ; une couche fichier mince (`writer.rs`, `reader.rs`) sur les traits `Disk` et `File` de `ndaal-io`.

**Dépendances :** `lz4_flex` (ADR-018) et `prost` avec `derive`, sans `prost-build`, sans `protoc` (ADR-015), versions dans `[workspace.dependencies]` ; `crc32c` par `ndaal-wal::crc`.

**Spécification :** [docs/superpowers/specs/2026-11-03-t0213-segment-envelope-design.md](../specs/2026-11-03-t0213-segment-envelope-design.md), validée le 2026-11-03. En cas de doute, elle fait foi : offsets, bornes et ordre des vérifications y sont fixés (§3).

## Contraintes globales

- Aucune E/S hors des traits de `ndaal-io` (ADR-017) ; aucune panique sur un octet lu du disque : toute donnée lue est bornée **avant** allocation (`MAX_BLOCK_LEN` = 16 Mio, `MAX_DIRECTORY_LEN` = 16 Mio).
- `HEADER_LEN` = `FOOTER_LEN` = `SECTION_ALIGN` = 4 096 ; `WRITE_BUFFER` = 1 Mio ; magiques `NDLSEG01` et `STRSEGND` ; `FORMAT` = 1.
- `#![forbid(unsafe_code)]` (déjà en place) ; ASCII seulement dans les `.rs` ; fichiers de 400 lignes au plus ; erreurs `thiserror` en anglais minuscule.
- Visibilité minimale : sont `pub` les types de §4 de la spécification et le codec pur dont le fuzzing a besoin (`SegmentHeader::{encode, decode}`, `Footer::{encode, decode}`, `encode_block`, `decode_block`, `decode_directory`).
- Test d'abord, le voir échouer ; mutation quand un test passe du premier coup ; tout défaut corrigé ajoute une ligne au registre des défauts.
- Toujours `export CARGO_TARGET_DIR="$PWD/target"` avant cargo.

## Points à surveiller en revue

1. **Une taille annoncée géante** (bloc LZ4, répertoire, offset de répertoire proche de `u64::MAX`) : erreur `Corrupt`, sans allocation ni débordement arithmétique (`checked_add`).
2. **Une poignée hors section** (`offset` dans le bourrage, à cheval sur deux sections, `len < 5`) : `Corrupt`, sans lecture hors du segment.
3. **Un bit inversé dans une zone réservée** de l'en-tête ou du pied : refusé, même si le CRC de la zone a été recalculé (le test recalcule le CRC pour isoler la règle).
4. **Un fichier tronqué ou rallongé** d'un nombre d'octets non multiple de 4 096 : refusé à l'étape 1 de l'ouverture.
5. **Un `create` sur un chemin existant** : erreur, le fichier existant intact.

---

### Tâche 1 : CRC ensemencé générique, en-tête et pied

**Fichiers :** modifier `crates/ndaal-wal/src/crc.rs` et `lib.rs` (réexport) ; créer `crates/ndaal-storage/src/segment/{mod.rs, format.rs}` ; modifier `crates/ndaal-storage/src/lib.rs` (`pub mod segment;`).

**Interfaces (produit) :**
- `ndaal_wal::seeded_crc_with(seed: &[u8], data: &[u8]) -> u32` ; `seeded_crc(epoch, file_no, data)` l'appelle avec sa graine de 16 octets (octets produits inchangés).
- `SegmentKind` (`#[repr(u16)]`, `TryFrom<u16>`), `SegmentHeader` (spéc. §4), `SegmentError` (spéc. §4.1).
- `SegmentHeader::encode(&self) -> [u8; HEADER_LEN]`, `SegmentHeader::decode(bytes: &[u8], expected: SegmentKind) -> Result<(SegmentHeader, u16), SegmentError>` (rend aussi `format`).
- `pub struct Footer { pub directory_offset: u64, pub directory_len: u32, pub directory_crc: u32, pub format: u16 }` ; `Footer::encode(&self) -> [u8; FOOTER_LEN]`, `Footer::decode(bytes: &[u8]) -> Result<Footer, SegmentError>`.

- [ ] **Tests d'abord** (`#[cfg(test)]` dans `format.rs`, et `crates/ndaal-wal/tests/reader.rs` pour la non-régression) :
  - `seeded_crc_is_unchanged` : `seeded_crc(7, 9, b"abc") == seeded_crc_with(&[7u64.to_le_bytes(), 9u64.to_le_bytes()].concat(), b"abc")`, et égal à la valeur figée calculée avant la modification ;
  - `header_round_trip`, `footer_round_trip` ;
  - `every_header_bit_flip_is_refused` / `every_footer_bit_flip_is_refused` : pour chaque bit des 4 096 octets, `decode` rend `Err` ;
  - `reserved_bytes_are_checked` : un octet réservé non nul, CRC recalculé, rend `Corrupt` ;
  - `format_kind_and_key_are_checked` : `format` = 2 rend `UnsupportedFormat { found: 2 }` ; un autre `kind` rend `WrongKind` ; `key_id` non nul rend `Encrypted` ; `min_lsn > max_lsn` rend `Corrupt` (chaque fois CRC recalculé).
- [ ] `cargo test -p ndaal-storage segment::format` et `cargo test -p ndaal-wal` réussissent.

### Tâche 2 : blocs

**Fichiers :** créer `crates/ndaal-storage/src/segment/block.rs` ; ajouter `lz4_flex` (sans `default-features` si `safe-encode`/`safe-decode` suffisent : pas d'`unsafe` dans la dépendance) au `Cargo.toml` racine et de `ndaal-storage`.

**Interfaces (produit) :**
- `pub enum Compression { None, Lz4 }` ;
- `pub fn block_seed(segment: SegmentId, offset: u64) -> [u8; 24]` (`segment_id` gros-boutiste, puis `offset` petit-boutiste) ;
- `pub fn encode_block(data: &[u8], compression: Compression, seed: &[u8; 24], out: &mut Vec<u8>) -> Result<(), SegmentError>` (ajoute à `out` ; `Misuse` si vide ou > `MAX_BLOCK_LEN` ; stockage brut si LZ4 ne réduit pas) ;
- `pub fn decode_block(stored: &[u8], seed: &[u8; 24]) -> Result<Vec<u8>, SegmentError>` (`stored` inclut les 5 octets de fin).

- [ ] **Tests d'abord** (`#[cfg(test)]` dans `block.rs`) :
  - `round_trip_both_compressions` ; `lz4_shrinks_repetitive_data` (1 Mio de zéros stocké en moins de 10 Kio) ; `incompressible_is_stored_raw` (octet de compression = 0) ;
  - `every_bit_flip_is_refused` sur un bloc de 300 octets, compressé et non compressé ;
  - `other_seed_is_refused` : autre `segment_id`, puis autre offset ;
  - `huge_announced_size_is_refused` : bloc LZ4 forgé annonçant `MAX_BLOCK_LEN + 1` octets, CRC valide, rend `Corrupt` ; `wrong_decompressed_size_is_refused` ;
  - `unknown_compression_is_refused` (octet 2, CRC valide) ; `empty_and_oversized_are_misuse`.
- [ ] `cargo test -p ndaal-storage segment::block` réussit.

### Tâche 3 : répertoire

**Fichiers :** créer `crates/ndaal-storage/src/segment/directory.rs` ; ajouter `prost` (fonctionnalités `derive` et `std` seulement) au `Cargo.toml` racine et de `ndaal-storage`.

**Interfaces (produit) :**
- messages `prost` privés `SectionDirectory`, `SectionEntry` (étiquettes de la spéc. §3.3) ;
- `pub struct Section` (spéc. §4) ;
- `pub(crate) fn encode_directory(sections: &[Section]) -> Vec<u8>` ;
- `pub fn decode_directory(bytes: &[u8], directory_offset: u64) -> Result<Vec<Section>, SegmentError>` : décode et vérifie les règles de la spéc. §3.3 (triées, alignées sur 4 096, disjointes, entre `HEADER_LEN` et `directory_offset`, `len > 0`, `blocks > 0`). La taille et le CRC sont vérifiés par l'appelant avant.

- [ ] **Tests d'abord** :
  - `directory_round_trip` (dont `properties` vide et non vide, zéro section) ;
  - `wire_tags_match_the_proto` : un `SectionEntry` encodé a les octets attendus calculés à la main pour `kind = 1, offset = 4096, len = 5, blocks = 1` (fixe les étiquettes 1 à 5) ;
  - `invalid_sections_are_refused` : chevauchement, désordre, non aligné, avant `HEADER_LEN`, au-delà de `directory_offset`, `offset + len` qui déborde `u64`, `len` nul : chacun `Corrupt` ;
  - `garbage_is_refused` : octets non protobuf rendent `Corrupt`, sans panique.
- [ ] `cargo test -p ndaal-storage segment::directory` réussit.

### Tâche 4 : écrivain et lecteur

**Fichiers :** créer `crates/ndaal-storage/src/segment/{writer.rs, reader.rs}` ; créer `crates/ndaal-sim/tests/segment.rs` (sur le modèle de `wal_files.rs` : `Sim`, `SimDisk`).

**Interfaces (consomme) :** tâches 1 à 3. **(produit) :** `SegmentWriter<F>`, `SegmentReader<F>`, `BlockHandle` : signatures de la spéc. §4.

- `create` ouvre en `create_new` ; le tampon commence à l'offset `HEADER_LEN` et se vide par `write_all_at` dès qu'il atteint `WRITE_BUFFER`.
- `begin_section` bourre de zéros jusqu'au multiple de 4 096 suivant ; la graine d'un bloc est `block_seed(segment_id, offset du bloc)`.
- `finish` : `Misuse` si une section est ouverte ; écrit le répertoire (graine `block_seed(segment_id, directory_offset)`), bourre jusqu'au multiple de 4 096, écrit le pied, puis l'en-tête à l'offset 0, puis `sync_data` ; rend la taille du fichier.
- `open` : ordre de la spéc. §3.5 ; lit le pied, l'en-tête, puis le répertoire (taille bornée avant lecture).
- `read_block` : `Corrupt` si `len < 5`, `len - 5 > MAX_BLOCK_LEN`, ou si `[offset, offset + len)` ne tient pas dans une section ; une lecture courte est `Corrupt`.

- [ ] **Tests d'abord** (`crates/ndaal-sim/tests/segment.rs`) :
  - `write_then_read` : trois sections (dont une compressée), poignées relues égales, `header()` et `sections()` égaux à l'écrit ;
  - `sections_are_aligned_and_size_is_a_multiple` ;
  - `misuse_is_an_error` : bloc hors section, `begin_section` deux fois, `finish` avec section ouverte, section vide ;
  - `create_never_overwrites` (point 5) ;
  - `handles_outside_sections_are_refused` (point 2) : poignée dans le bourrage, à cheval, `len = 4`, `offset` proche de `u64::MAX` ;
  - `odd_file_sizes_are_refused` (point 4) : fichier rallongé d'un octet, tronqué de 4 096 ;
  - `property::random_segments_round_trip` (`proptest`, 1 à 5 sections de 1 à 20 blocs de 1 à 40 000 octets, compressions aléatoires).
- [ ] `cargo test -p ndaal-sim --test segment` réussit.

### Tâche 5 : corruption, pannes, fuzzing

**Fichiers :** `crates/ndaal-sim/tests/segment.rs` ; `crates/ndaal-fuzz/src/lib.rs` (et `Cargo.toml` : `ndaal-storage`) ; créer `fuzz/fuzz_targets/segment_reader.rs` et son `[[bin]]` dans `fuzz/Cargo.toml` ; graine de corpus `fuzz/corpus/segment_reader/`.

- [ ] **Tests d'abord** :
  - `every_corrupted_bit_is_detected` : segment de deux sections et quatre blocs ; pour chaque offset du fichier, `SimDisk::corrupt` sur une copie, puis `open` et lecture de toutes les poignées : soit une erreur, soit (offset dans un bourrage) des blocs égaux à l'original ; au moins une erreur pour tout offset hors bourrage ;
  - `crash_before_finish_is_refused` : `crash(LoseAllUnsynced)` après des `add_block` sans `finish`, puis `open` rend `Err` ; idem `crash(Random)` pour 20 graines pendant le `sync_data` de `finish` : `open` rend `Err` ou le segment complet ;
  - `io_errors_surface` : `fail_next` pendant `finish`, puis pendant `read_block`, rend `SegmentError::Io`.
- [ ] **Fuzzing** : corps `pub fn segment_reader(data: &[u8])` ajouté à `TARGETS` : sur `data` comme fichier entier, `Footer::decode` du dernier `FOOTER_LEN`, `SegmentHeader::decode` du premier, `decode_directory` de la tranche indiquée si elle tient, et `decode_block` de `data` avec une graine fixe ; aucune panique ; tout bloc décodé ré-encode puis se décode à l'identique. Corpus versionné `fuzz/corpus/segment_reader/` : `valid-two-sections` (segment valide écrit une fois par `SegmentWriter` sur `RealDisk` depuis un programme jetable, non versionné) et `torn-footer` (le même, tronqué de 100 octets) ; deux lignes au README du corpus. Passe courte : `cargo +nightly fuzz run segment_reader -- -max_total_time=60` dans `fuzz/`.
- [ ] `cargo test -p ndaal-sim --test segment` et `cargo test -p ndaal-fuzz` réussissent.

### Tâche 6 : documents et vérifications

**Fichiers :** `FORMATS_DE_DONNEES.md` §3 ; `DECISIONS.md` (ADR-035) ; `ARCHITECTURE.md` §3 ; `POINTS_REPORTES.md` ; `docs/recherche/registre-des-defauts.md` si un défaut a été corrigé .

- [ ] **Documents** :
  - FORMATS §3 : tableaux d'offsets de la spéc. §3.1 et §3.4, alignement des sections, graine des CRC, `MAX_BLOCK_LEN`, ordre de l'ouverture ;
  - ADR-035 « CRC ensemencés des segments » : décision, raison, coût (pas de recopie brute de bloc), renvoi à ADR-034 ;
  - ARCHITECTURE §3 : une ligne `ndaal-storage::segment` ;
  - POINTS_REPORTES : métrique de corruption (à l'appelant), chiffrement (T11.07), Zstandard.
- [ ] **Vérifications** : chaîne de CLAUDE.md §11 avec `CARGO_TARGET_DIR=<dépôt>/target`, dont `check-deny.sh` (licences de `lz4_flex` et `prost`) et `check-linux.sh` ; revue finale par un relecteur neuf ; compte rendu, `.git/pr-body.md`, commandes git et `gh pr create`.
- [ ] Cocher T02.13 après la CI verte.
