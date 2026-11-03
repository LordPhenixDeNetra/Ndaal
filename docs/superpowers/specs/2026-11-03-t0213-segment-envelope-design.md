# T02.13 — Enveloppe de segment générique : spécification

**Tâche :** T02.13 ([CAHIER_DES_TACHES.md](../../../CAHIER_DES_TACHES.md#j02--noyau-de-stockage)).

**Exigences :** ENF-DUR-03, ENF-QUAL-03, ADR-015, ADR-018, ADR-035 (nouveau).

**Sources :**
- [FORMATS §3](../../../FORMATS_DE_DONNEES.md#3-fichier-de-segment-générique), complété par cette spécification ;
- [ADR-034](../../../DECISIONS.md#adr-034--crc-den-tête-et-crc-ensemencées-du-journal), dont ADR-035 reprend la logique.

**Statut :** conception validée par le responsable du projet le 2026-11-03, section par section.

## 1. Contexte et périmètre

Tous les segments immuables partagent une enveloppe : la SST clé-valeur (T02.15), les segments vectoriels (T05.10), les métadonnées objet, les valeurs volumineuses et les charges utiles. Le moteur produit les octets de ses sections ; l'enveloppe les range dans le fichier, les protège par CRC, les compresse, et les relit en vérifiant tout.

Le graphe des crates interdit les E/S dans `ndaal-kv` et `ndaal-vector`, qui ne dépendent que de `ndaal-common`. L'enveloppe vit donc dans **`ndaal-storage::segment`**, et lit et écrit par le trait `Disk` de `ndaal-io` (ADR-017).

**Critère de fin (cahier des tâches) :** tout bloc corrompu est détecté à la lecture, et les numéros de format sont vérifiés à l'ouverture.

**Hors du périmètre :**
- le chiffrement (T11.07) : un `key_id` non nul est refusé à l'ouverture, jamais lu comme du clair ;
- Zstandard (optionnel, ADR-018) ;
- la publication atomique, c'est-à-dire le renommage et le `fsync` du répertoire (T02.16) : un segment n'existe pour le moteur qu'une fois cité par le manifeste ;
- le cache de blocs et la projection en mémoire (`mmap`, T05.10) ;
- la métrique de corruption : l'appelant compte les erreurs `Corrupt` (point reporté).

## 2. Décisions

| # | Décision | Raison | Coût |
|---|---|---|---|
| 1 | **CRC des blocs et du répertoire ensemencé par `segment_id` et l'offset** (ADR-035) | Un bloc lu au mauvais endroit (bogue d'index) ou venu d'un autre segment échoue, alors qu'il est valide en lui-même | Un bloc ne peut pas être recopié tel quel d'un segment à l'autre : son CRC doit être recalculé (bien moins cher qu'une décompression) |
| 2 | **Chaque section commence à un offset multiple de 4 096** | Lecture en `O_DIRECT` et projection en mémoire d'une section sans recopie (T05.10) | Environ 2 Kio de bourrage en moyenne par section ; une dizaine de sections au plus par segment |
| 3 | **Les blocs d'une section ne sont pas alignés** | Aligner chaque bloc compressé de 16 Kio de la SST gaspillerait environ 12 % | — |
| 4 | **Codec pur et couche fichier mince** | Le fuzzing vise les fonctions pures ; la couche fichier passe par le simulateur | — |
| 5 | **Écriture en flux, tampon de 1 Mio** | Mémoire bornée quelle que soit la taille du segment (P8) | Le segment n'est pas relisible avant `finish` |
| 6 | **Répertoire en protobuf avec `prost`, messages dérivés à la main** | ADR-015 ; `prost-build` exigerait `protoc`, un binaire C++ (C-01) | Les messages `.proto` ne sont pas générés : la correspondance est vérifiée par test |
| 7 | **Le pied gagne un CRC et un champ `format`** | FORMATS §3.4 n'en prévoyait pas : un offset de répertoire corrompu ne serait détecté qu'indirectement | 6 octets pris sur la zone réservée |
| 8 | **Toute zone réservée est vérifiée nulle** | Une version future qui l'utilise est refusée franchement plutôt que mal lue | — |

## 3. Format sur disque

Entiers en petit-boutiste, sauf `segment_id`, en gros-boutiste (FORMATS §1 et §10).

```
0        en-tete (4 096 octets)
4096     section 1 : blocs              <- offset multiple de 4 096
         bourrage (zeros)
         section 2 : blocs              <- offset multiple de 4 096
         ...
         repertoire (protobuf)          <- juste apres la derniere section
         bourrage (zeros)
taille-4096  pied (4 096 octets)        <- offset multiple de 4 096
```

### 3.1 En-tête (4 096 octets, offset 0)

| Offset | Taille | Champ | Règle à l'ouverture |
|---|---|---|---|
| 0 | 8 | `magic` = `NDLSEG01` | égal |
| 8 | 2 | `format` = 1 | connu, sinon `UnsupportedFormat` |
| 10 | 2 | `kind` (1 SST, 2 vectoriel, 3 métadonnées objet, 4 valeurs volumineuses, 5 charge utile) | égal au type attendu, sinon `WrongKind` |
| 12 | 2 | `kind_format` | rendu sans interprétation : le moteur le vérifie |
| 14 | 2 | réservé | nul |
| 16 | 16 | `segment_id` | — |
| 32 | 4 | `partition_id` | — |
| 36 | 8 | `min_lsn` | `min_lsn ≤ max_lsn` |
| 44 | 8 | `max_lsn` | — |
| 52 | 16 | `key_id` | nul, sinon `Encrypted` (jusqu'à T11.07) |
| 68 | 4 024 | réservé | nul |
| 4 092 | 4 | CRC masqué des octets 0 à 4 091 | égal |

Le CRC de l'en-tête n'est pas ensemencé : c'est lui qui porte le `segment_id`.

### 3.2 Bloc

```
donnees stockees (n) . compression (1) . crc (4)
```

- `compression` : `0` aucune, `1` LZ4. Toute autre valeur est une corruption.
- Avec LZ4, les données stockées sont la taille décompressée (4 octets) suivie du flux LZ4 (`lz4_flex`). Cette taille est comparée à `MAX_BLOCK_LEN` **avant** toute allocation, et la décompression doit produire exactement cette taille.
- `crc` : CRC32C masqué, ensemencé par `segment_id (16) · offset du bloc (8)`, sur les données stockées et l'octet de compression (ADR-035).
- `MAX_BLOCK_LEN` = 16 Mio, en taille décompressée comme en taille stockée. Un bloc vide est refusé à l'écriture.
- À l'écriture, si LZ4 ne réduit pas la taille, le bloc est stocké sans compression.

Une **poignée** `BlockHandle { offset, len }` désigne un bloc : `len` compte les données stockées et les 5 octets de fin. Le moteur garde ses poignées (par exemple dans l'index de la SST), et l'enveloppe ne connaît pas les blocs d'une section.

### 3.3 Répertoire

Protobuf, messages dérivés à la main avec `prost` :

```proto
message SectionDirectory { repeated SectionEntry sections = 1; }
message SectionEntry {
  uint32 kind = 1;        // propre au type de segment
  uint64 offset = 2;      // multiple de 4 096
  uint64 len = 3;         // octets de blocs, sans le bourrage
  uint32 blocks = 4;
  bytes properties = 5;   // propres au moteur, opaques pour l'enveloppe
}
```

À l'ouverture, les règles suivantes sont vérifiées :
- la taille du répertoire est d'au plus 16 Mio ;
- son CRC est ensemencé par `segment_id` et son offset ;
- ses sections sont triées par offset, alignées et disjointes ;
- elles sont comprises entre la fin de l'en-tête et le début du répertoire.

### 3.4 Pied (4 096 octets, en fin de fichier)

| Offset | Taille | Champ | Règle à l'ouverture |
|---|---|---|---|
| 0 | 8 | offset du répertoire | après la dernière section, avant le pied |
| 8 | 4 | taille du répertoire | ≤ 16 Mio, tient avant le pied |
| 12 | 4 | CRC du répertoire (ensemencé) | vérifié avec le répertoire |
| 16 | 2 | `format` | égal à celui de l'en-tête |
| 18 | 4 066 | réservé | nul |
| 4 084 | 4 | CRC masqué des octets 0 à 4 083 | égal |
| 4 088 | 8 | `magic` = `STRSEGND` | égal |

### 3.5 Ouverture

Les vérifications se font dans cet ordre, et la première qui échoue rend une erreur typée :
1. la taille du fichier est au moins de 8 Kio et multiple de 4 096 ;
2. le pied est valide ;
3. l'en-tête est valide, et le type de segment est celui attendu ;
4. les deux versions de format sont égales ;
5. le répertoire est valide.

Un fichier tronqué, par une coupure avant la fin de `finish`, échoue à l'étape 1 ou 2.

## 4. Interface (crate `ndaal-storage`, module `segment`)

```rust
pub enum SegmentKind { Sst = 1, Vector = 2, ObjectMeta = 3, LargeValues = 4, Payload = 5 }
pub enum Compression { None, Lz4 }
pub struct SegmentHeader {
    pub kind: SegmentKind,
    pub kind_format: u16,
    pub segment_id: SegmentId,
    pub partition: PartitionId,
    pub min_lsn: Lsn,
    pub max_lsn: Lsn,
}
pub struct BlockHandle { pub offset: u64, pub len: u32 }
pub struct Section { pub kind: u32, pub offset: u64, pub len: u64, pub blocks: u32,
                     pub properties: Vec<u8> }

impl<F: File> SegmentWriter<F> {
    pub async fn create<D: Disk<File = F>>(disk: &D, path: &Path, header: SegmentHeader)
        -> Result<Self, SegmentError>;  // sonde puis cree : n'ecrase jamais ; Misuse si min_lsn > max_lsn
    pub fn begin_section(&mut self, kind: u32) -> Result<(), SegmentError>;  // bourrage en memoire
    pub async fn add_block(&mut self, data: &[u8], compression: Compression)
        -> Result<BlockHandle, SegmentError>;
    pub fn end_section(&mut self, properties: Vec<u8>) -> Result<(), SegmentError>;
    pub async fn finish(self) -> Result<u64, SegmentError>;  // taille du fichier
}

impl<F: File> SegmentReader<F> {
    pub async fn open<D: Disk<File = F>>(disk: &D, path: &Path, expected: SegmentKind)
        -> Result<Self, SegmentError>;
    pub fn header(&self) -> &SegmentHeader;
    pub fn sections(&self) -> &[Section];
    pub async fn read_block(&self, handle: BlockHandle) -> Result<Vec<u8>, SegmentError>;
}
```

- `finish` écrit le bourrage, le répertoire et le pied, puis l'en-tête à l'offset 0, puis fait `sync_data`. Il ne renomme rien.
- `read_block` vérifie d'abord que la poignée tient entièrement dans une section du répertoire : une poignée corrompue venue d'un index échoue en `Corrupt`, sans lecture hors du segment.
- Un usage incohérent de l'écrivain rend `Misuse`, jamais une panique :
  - un bloc hors section, ou une section ouverte deux fois ;
  - une section vide ou non fermée au moment de `finish` ;
  - un bloc vide ou trop grand.
- Le CRC ensemencé est généralisé dans `ndaal-wal::crc` par `seeded_crc_with(seed: &[u8], data)`. `seeded_crc` du journal l'appelle avec sa graine, et les octets produits par le journal ne changent pas (test de non-régression).

### 4.1 Erreurs

```rust
pub enum SegmentError {
    Io(#[source] std::io::Error),
    Corrupt { what: &'static str, offset: u64 },
    UnsupportedFormat { found: u16 },
    WrongKind { expected: SegmentKind, found: u16 },
    Encrypted,
    Misuse(&'static str),
}
```

## 5. Organisation du code

| Fichier | Rôle |
|---|---|
| `segment/mod.rs` | Types publics, `SegmentError` |
| `segment/format.rs` | Pur : en-tête et pied, constantes |
| `segment/block.rs` | Pur : `encode_block`, `decode_block`, LZ4, bornes |
| `segment/directory.rs` | Pur : messages `prost`, validation |
| `segment/writer.rs` | `SegmentWriter` |
| `segment/reader.rs` | `SegmentReader` |

Les nouvelles dépendances `lz4_flex` et `prost` (sans `prost-build`) sont toutes deux en Rust pur. Leurs licences sont vérifiées par `cargo deny`.

## 6. Tests

**Unitaires, sur la partie pure :**
- En-tête et pied : aller-retour, puis inversion de chaque bit un par un. Toute inversion rend une erreur typée, jamais `Ok`.
- Bloc :
  - aller-retour avec et sans LZ4 ;
  - stockage brut quand LZ4 ne gagne rien ;
  - inversion de chaque bit ;
  - refus avec un autre `segment_id` ou un autre offset ;
  - taille décompressée annoncée au-delà de `MAX_BLOCK_LEN` refusée avant allocation.
- Répertoire : sections qui se chevauchent, hors du fichier, non triées, non alignées ou trop grandes, toutes refusées.

**Propriété (`proptest`, modèle naïf) :**
- Des sections et des blocs aléatoires, avec des compressions variées, sont écrits puis relus : chaque bloc relu est égal à l'original, et le répertoire relu est égal au répertoire écrit.
- Chaque section commence à un offset multiple de 4 096, et la taille du fichier est multiple de 4 096.

**Simulation (`crates/ndaal-sim/tests/segment.rs`) :**
- Écriture puis relecture sur `SimDisk`.
- Corruption exhaustive d'un petit segment : on inverse un bit à chaque offset, puis on ouvre et on lit tous les blocs. Le résultat est soit une erreur typée, soit, si le bit est dans un bourrage jamais lu, une lecture identique à l'original. C'est la preuve du critère de fin.
- Coupure avant `finish` ou pendant son `sync_data` : le fichier est refusé à l'ouverture.
- Une erreur d'E/S du disque simulé remonte en `Io`.
- `create` sur un chemin existant échoue.

**Fuzzing :**
- un corps `segment_reader` dans `crates/ndaal-fuzz` (décodage de l'en-tête, du pied, du répertoire et d'un bloc sur des octets arbitraires), rejoué sur stable ;
- la cible `fuzz/fuzz_targets/segment_reader.rs`, avec une passe courte en local.

## 7. Documents

- FORMATS §3 : offsets exacts, pied avec CRC et `format`, alignement des sections, CRC ensemencés.
- DECISIONS : ADR-035, CRC ensemencés des segments.
- ARCHITECTURE §3 : le module `ndaal-storage::segment`.
- POINTS_REPORTES : métrique de corruption, chiffrement (T11.07), Zstandard.
- Registre des défauts : une ligne par défaut corrigé.
