# Formats de données — Ndaal

> Spécification des formats binaires sur disque et des identifiants. Toute modification d'un format DOIT incrémenter son numéro de format et être accompagnée d'un test de lecture des anciens formats (ENF-COMPAT-04).

## Sommaire

1. [Conventions](#1-conventions)
2. [Journal (WAL)](#2-journal-wal)
3. [Fichier de segment générique](#3-fichier-de-segment-générique)
4. [Segment SST clé-valeur](#4-segment-sst-clé-valeur)
5. [Segment vectoriel](#5-segment-vectoriel)
6. [Segment de métadonnées objet](#6-segment-de-métadonnées-objet)
7. [Blocs d'objets](#7-blocs-dobjets)
8. [Manifeste de partition](#8-manifeste-de-partition)
9. [Encodage des clés internes](#9-encodage-des-clés-internes)
10. [Identifiants](#10-identifiants)
11. [Catalogue](#11-catalogue)
12. [Chiffrement](#12-chiffrement)
13. [Arborescence sur disque](#13-arborescence-sur-disque)
14. [Politique de compatibilité](#14-politique-de-compatibilité)

---

## 1. Conventions

| Élément | Convention |
|---|---|
| Ordre des octets | Petit-boutiste (*little-endian*) pour tous les entiers de taille fixe. Exception : les identifiants composés de 16 octets (`segment_id`, `chunk_id`, `version_id`) sont gros-boutistes, pour que l'ordre de leurs octets suive l'ordre de leurs valeurs (§10) |
| Entiers variables | `varint` = LEB128 non signé ; `zigzag` pour les signés |
| Chaînes | `varint` longueur + octets UTF-8 (clés utilisateur : octets arbitraires) |
| Sommes de contrôle | CRC32C (Castagnoli), accéléré matériellement (crate `crc32c`) ; masqué comme dans LevelDB (`((crc >> 15) | (crc << 17)) + 0xa282ead8`) pour éviter les collisions avec des données contenant leur propre CRC |
| Empreintes de contenu | xxHash3-128 pour les identifiants de contenu internes ; MD5/SHA-256/CRC uniquement pour la compatibilité S3 |
| Alignement | Fichiers de journal et de segment alignés sur 4 096 octets (compatibilité `O_DIRECT`) |
| Nombres magiques | 8 octets ASCII en tête et en fin de fichier |
| Version de format | `u16` après le nombre magique |
| Compression | Indicateur par bloc : `0` aucune, `1` LZ4 (`lz4_flex`), `2` Zstandard (optionnel) |
| Horodatages | `u64` microsecondes depuis l'époque Unix, horloge du leader |

---

## 2. Journal (WAL)

### 2.1 Fichiers

```
<data_dir>/wal/shard-<NN>/<numéro de fichier sur 16 chiffres hexadécimaux>.wal
exemple : data/wal/shard-03/00000000000000a2.wal
```

- Taille cible : 64 Mio (configurable). Préalloués (`fallocate`) à la création pour éviter les mises à jour de métadonnées du système de fichiers lors des `fdatasync`.
- Un fichier est **recyclé** (renommé et réutilisé) plutôt que supprimé quand c'est possible.
- Un fichier libéré, ou en préparation, s'appelle `free-<numéro sur 16 chiffres hexadécimaux>.wal`. Un fichier ne prend son numéro qu'une fois son en-tête écrit et synchronisé : il est préparé sous son nom libre, puis renommé, suivi d'un `fsync` du répertoire. Un arrêt au milieu laisse un fichier libre, jamais un fichier numéroté invalide.
- La fin d'un fichier est remplie par des enregistrements `PADDING` : l'écrivain ne laisse jamais entre 1 et 43 octets libres. Un fichier qui n'est pas le dernier se lit donc toujours jusqu'à sa fin exacte.

### 2.2 En-tête de fichier (4 096 octets)

| Décalage | Taille | Champ | Description |
|---|---|---|---|
| 0 | 8 | `magic` | `NDLWAL01` |
| 8 | 2 | `format` | Version de format du journal (actuellement 1) |
| 10 | 2 | `shard_id` | Numéro du shard |
| 12 | 4 | `flags` | Bit 0 : chiffré ; bit 1 : fichier recyclé |
| 16 | 8 | `file_no` | Numéro du fichier |
| 24 | 16 | `node_id` | Identifiant du nœud (UUID) |
| 40 | 8 | `created_at` | Horodatage |
| 48 | 8 | `recycle_epoch` | Incrémenté à chaque recyclage ; recopié dans chaque enregistrement pour distinguer les anciens enregistrements d'un fichier recyclé |
| 56 | 16 | `key_id` | Identifiant de la clé de chiffrement (si chiffré) |
| 72 | 4020 | réservé | Zéros |
| 4092 | 4 | `crc` | CRC32C masqué des octets 0–4091 |

### 2.3 Trame d'enregistrement

Les enregistrements se suivent sans alignement individuel.

| Taille | Champ | Description |
|---|---|---|
| 4 | `crc` | CRC32C masqué et ensemencé de tout ce qui suit (en-tête restant + charge utile) |
| 4 | `header_crc` | CRC32C masqué et ensemencé des 36 octets d'en-tête qui suivent (`len` à `timestamp`) |
| 4 | `len` | Longueur de la charge utile |
| 1 | `type` | Type d'enregistrement (§2.4) |
| 1 | `flags` | Bit 0 : charge utile compressée ; bit 1 : chiffrée ; bit 2 : dernier fragment ; bit 3 : premier fragment |
| 2 | `recycle_epoch` | 16 bits de poids faible de l'époque de recyclage (filtre rapide) |
| 4 | `partition_id` | Partition concernée |
| 8 | `lsn` | LSN de la partition |
| 8 | `term` | Terme Raft (0 hors cluster) |
| 8 | `timestamp` | Horloge du leader |
| `len` | `payload` | Charge utile (format dépendant du type) |

En-tête : 44 octets. Les deux CRC sont **ensemencées** ([ADR-034](DECISIONS.md#adr-034--crc-den-tête-et-crc-ensemencées-du-journal)) : 16 octets formés de l'époque de recyclage complète (u64) puis du `file_no` (u64) de l'en-tête du fichier sont placés devant les données couvertes. Une trame d'une autre vie du fichier, ou d'un autre fichier, échoue donc à la CRC, quelle que soit son époque basse. La CRC d'en-tête permet d'écarter un faux candidat en 44 octets, avant de lire la charge utile. Un enregistrement dont la charge utile dépasse 1 Mio est **fragmenté** en trames de 1 Mio au plus (bits 2 et 3 ; une trame entière porte les deux). Les fragments sont consécutifs, et peuvent passer d'un fichier au suivant. Un premier fragment qui arrive alors qu'un enregistrement fragmenté est incomplet signifie que celui-ci a été déchiré par une coupure, puis que l'écrivain a repris : l'enregistrement incomplet est abandonné (il n'a jamais été acquitté, puisque l'acquittement suit la synchronisation de son dernier fragment). Les drapeaux « compressé » et « chiffré » ne sont pas encore écrits ; une trame qui les porte est refusée, jamais lue de travers. Les bits 4 à 7 sont réservés et valent zéro.

**Écritures interrompues** ([ADR-033](DECISIONS.md#adr-033--zone-déchirée-bornée-du-journal)) : à la relecture, le premier enregistrement dont la CRC est invalide, la longueur absurde, le type inconnu ou l'époque de recyclage incorrecte marque la **fin du journal valide**, sauf si une trame valide commence entre **2 Mio (`MAX_FLUSH`) et 2 Mio + 1 Mio + 44 octets plus loin** dans le fichier : il s'agit alors d'une **corruption** (§6.6 de l'architecture). L'écrivain n'a jamais qu'un vidage en cours, qui finit sur une frontière de trame et tient en 2 Mio page réécrite comprise, et n'acquitte qu'après son `fdatasync` : ce qu'une coupure a déchiré n'a jamais été acquitté.

**`O_DIRECT`** : les écritures sont des multiples de 4 096 octets. La dernière page partielle est complétée par des zéros sur disque et **réécrite** au vidage suivant avec les nouveaux enregistrements à la suite (les octets déjà écrits sont identiques).

### 2.4 Types d'enregistrement

| Code | Type | Charge utile |
|---|---|---|
| `0x01` | `KV_EFFECTS` | Liste d'effets clé-valeur (§2.5) |
| `0x02` | `VEC_EFFECTS` | Liste d'effets vectoriels |
| `0x03` | `OBJ_EFFECTS` | Liste d'effets objet (métadonnées) |
| `0x04` | `TX` | Transaction : liste mixte d'effets des trois modèles, avec identifiant de transaction |
| `0x10` | `CHECKPOINT_BEGIN` | Début de point de contrôle d'une partition (LSN) |
| `0x11` | `CHECKPOINT_END` | Fin de point de contrôle (identifiant de manifeste) |
| `0x20` | `SCHEMA` | Changement de catalogue local à la partition (collection créée, index ajouté…) |
| `0x21` | `SPLIT` | Division de partition |
| `0x22` | `MERGE` | Fusion de partitions |
| `0x30` | `RAFT_MEMBERSHIP` | Changement de membres du groupe Raft |
| `0x31` | `RAFT_BLANK` | Entrée vide d'un nouveau leader |
| `0x40` | `EXPIRE_DEL` | Suppressions par expiration (liste de clés) |
| `0x41` | `GC_MARK` | Point de marquage du ramasse-miettes des blocs |
| `0xF0` | `PADDING` | Bourrage (ignoré) |

### 2.5 Effets

Chaque effet commence par un octet de **code d'effet**, suivi de champs encodés en `varint` / chaînes.

**Clé-valeur** (`model = 0`) :

| Code | Effet | Champs |
|---|---|---|
| `0x01` | `SET` | `db`, `key`, `value_type`, `value` (encodage du type), `expire_at` (0 = aucun), `version` |
| `0x02` | `DEL` | `db`, `key`, `version` |
| `0x03` | `EXPIRE` | `db`, `key`, `expire_at`, `version` |
| `0x04` | `LIST_PUSH` | `db`, `key`, `side`, `items[]`, `version` |
| `0x05` | `LIST_POP` | `db`, `key`, `side`, `count`, `version` |
| `0x06` | `HASH_SET` | `db`, `key`, `fields[] (champ, valeur)`, `version` |
| `0x07` | `HASH_DEL` | `db`, `key`, `fields[]`, `version` |
| `0x08` | `SET_ADD` / `0x09` `SET_REM` | `db`, `key`, `members[]`, `version` |
| `0x0A` | `ZSET_ADD` | `db`, `key`, `entries[] (score f64, membre)`, `version` |
| `0x0B` | `ZSET_REM` | `db`, `key`, `members[]`, `version` |
| `0x0C` | `RENAME` | `db`, `from`, `to`, `version` |
| `0x0D` | `FLUSH` | `db` |
| `0x0E` | `STREAM_ADD` … `0x13` | Effets des Streams (identifiant d'entrée explicite) |
| `0x20` | `DEL_PREFIX` | `db`, `prefix` (expansion d'une règle d'invalidation) |

Règle : pour toute opération dont le résultat dépend de l'état (incréments, `LSET`, `LINSERT`, `LREM`, `SPOP` aléatoire…), l'effet journalisé est **le résultat** (`SET` de la nouvelle valeur pour une chaîne, ou opération déterministe avec positions et éléments explicites pour les structures).

**Vecteurs** (`model = 1`) :

| Code | Effet | Champs |
|---|---|---|
| `0x01` | `UPSERT` | `collection_id`, `point_id`, `vectors[] (nom_id, type, données brutes)`, `payload` (JSON binaire), `links[]`, `version` |
| `0x02` | `DELETE` | `collection_id`, `point_ids[]`, `version` |
| `0x03` | `SET_PAYLOAD` | `collection_id`, `point_id`, `patch` (fusion), `version` |
| `0x04` | `DELETE_PAYLOAD_KEYS` | `collection_id`, `point_id`, `keys[]`, `version` |
| `0x05` | `UPDATE_VECTORS` | `collection_id`, `point_id`, `vectors[]`, `version` |

**Objets** (`model = 2`) :

| Code | Effet | Champs |
|---|---|---|
| `0x01` | `PUT` | `bucket_id`, `key`, `version_id`, `meta` (protobuf `ObjectMeta`), `inline_data` ou `chunks[]` |
| `0x02` | `DELETE` | `bucket_id`, `key`, `version_id` (ou marqueur de suppression) |
| `0x03` | `STAGE` | `staged_id`, `meta`, `chunks[]`, `expires_at` |
| `0x04` | `COMMIT_STAGED` | `staged_id`, `bucket_id`, `key`, `version_id` |
| `0x05` | `MPU_CREATE` / `0x06` `MPU_PART` / `0x07` `MPU_COMPLETE` / `0x08` `MPU_ABORT` | Multipart |
| `0x09` | `SET_TAGS` / `0x0A` `SET_RETENTION` | Étiquettes, verrouillage |

### 2.6 Enregistrement `TX`

```
tx_id        : u128
client_id    : varint (pour la déduplication des réessais, voir ci-dessous)
client_seq   : varint
nb_effets    : varint
effets[]     : { model: u8, code: u8, champs… }
```

**Déduplication** : un client peut fournir un identifiant d'idempotence ; la partition conserve les 10 000 derniers `(client_id, client_seq) → résultat` (journalisés via les `TX`) pour qu'un réessai après délai d'attente ne réapplique pas la transaction.

---

## 3. Fichier de segment générique

Tous les segments (SST, vectoriels, métadonnées objet) partagent une **enveloppe** commune.

```
┌──────────────────────────────────┐ 0
│ En-tête (4 096 octets)           │
├──────────────────────────────────┤
│ Section 1 (blocs)                │
│ Section 2 (blocs)                │
│ …                                │
├──────────────────────────────────┤
│ Répertoire des sections          │
├──────────────────────────────────┤
│ Pied de fichier (4 096 octets)   │
└──────────────────────────────────┘
```

Chaque section commence à un offset **multiple de 4 096** : l'écrivain bourre de zéros jusqu'à la frontière suivante. Une section peut ainsi être lue en `O_DIRECT` ou projetée en mémoire sans recopie. Les blocs d'une section, eux, se suivent sans alignement. Le répertoire suit la dernière section ; un bourrage le sépare du pied, lui aussi aligné. Implémentation : `ndaal-storage::segment` (T02.13).

### 3.1 En-tête (4 096 octets, offset 0)

| Offset | Taille | Champ | Description |
|---|---|---|---|
| 0 | 8 | `magic` | `NDLSEG01` |
| 8 | 2 | `format` | Version de l'enveloppe (1) |
| 10 | 2 | `kind` | `1` SST clé-valeur, `2` vectoriel, `3` métadonnées objet, `4` valeurs volumineuses, `5` charge utile |
| 12 | 2 | `kind_format` | Version du format spécifique, vérifiée par le moteur |
| 14 | 2 | réservé | Nul |
| 16 | 16 | `segment_id` | Identifiant (§10), gros-boutiste |
| 32 | 4 | `partition_id` | |
| 36 | 8 | `min_lsn` | Plage de LSN couverte, `min_lsn ≤ max_lsn` |
| 44 | 8 | `max_lsn` | |
| 52 | 16 | `key_id` | Clé de chiffrement ; zéros si non chiffré (seul cas accepté avant T11.07) |
| 68 | 4 024 | réservé | Nul |
| 4 092 | 4 | `crc` | CRC masqué des octets 0 à 4 091 |

### 3.2 Bloc

Chaque section est une suite de blocs :

| Taille | Champ |
|---|---|
| `n` | Données stockées : brutes, ou `taille décompressée (4) · flux LZ4` |
| 1 | Type de compression (`0` aucune, `1` LZ4 ; toute autre valeur est une corruption) |
| 4 | CRC32C masqué, **ensemencé** par `segment_id (16, gros-boutiste) · offset du bloc (8)`, sur les données stockées et l'octet de compression ([ADR-035](DECISIONS.md#adr-035--crc-ensemencées-des-segments)) |

- `MAX_BLOCK_LEN` = 16 Mio, en taille stockée comme décompressée ; la taille annoncée d'un bloc LZ4 est vérifiée **avant** toute allocation, et la décompression doit la produire exactement.
- L'écrivain stocke un bloc sans compression quand LZ4 ne réduit pas sa taille.
- Le moteur garde la position de ses blocs (`offset`, longueur stockée avec les 5 octets de fin) ; l'enveloppe refuse de lire un bloc qui ne tient pas entièrement dans une section.

Si chiffré (T11.07), les données stockées seront `nonce (12) || chiffré || tag (16)`.

### 3.3 Répertoire des sections

Encodé en protobuf (`SectionDirectory`, messages dérivés à la main avec `prost`, sans `protoc`) :

```proto
message SectionDirectory { repeated SectionEntry sections = 1; }
message SectionEntry {
  uint32 kind = 1;        // propre au type de segment
  uint64 offset = 2;      // multiple de 4 096
  uint64 len = 3;         // octets de blocs, sans le bourrage
  uint32 blocks = 4;
  bytes properties = 5;   // propres au moteur
}
```

Taille d'au plus 16 Mio. Les sections sont triées par offset, alignées, disjointes, non vides, et comprises entre la fin de l'en-tête et le début du répertoire.

### 3.4 Pied de fichier (4 096 octets, en fin de fichier)

| Offset | Taille | Champ |
|---|---|---|
| 0 | 8 | Décalage du répertoire |
| 8 | 4 | Taille du répertoire |
| 12 | 4 | CRC du répertoire, ensemencée par `segment_id · offset du répertoire` |
| 16 | 2 | `format`, égal à celui de l'en-tête |
| 18 | 4 066 | réservé, nul |
| 4 084 | 4 | CRC masqué des octets 0 à 4 083 |
| 4 088 | 8 | `magic` : `STRSEGND` |

### 3.5 Ouverture

Dans cet ordre, la première vérification qui échoue rend une erreur typée :
1. taille du fichier d'au moins 8 Kio et multiple de 4 096 (un fichier coupé avant la fin de l'écriture échoue ici ou à l'étape 2) ;
2. pied valide (magique, CRC, format, zones réservées) ;
3. en-tête valide et du type attendu (magique, CRC, format, zones réservées, `key_id` nul, plage de LSN) ;
4. même `format` dans l'en-tête et le pied ;
5. répertoire valide (bornes, taille, CRC, règles du §3.3).

Chaque bloc est ensuite vérifié **à chaque lecture**. Toute zone réservée est vérifiée nulle : une version future qui l'utilise est refusée franchement plutôt que mal lue.

---

## 4. Segment SST clé-valeur

| Section | Contenu |
|---|---|
| `DATA` | Blocs de 16 Kio (avant compression) d'entrées triées par clé interne (§9). Compression des préfixes de clé avec **points de redémarrage** tous les 16 entrées. |
| `INDEX` | Pour chaque bloc de données : dernière clé du bloc (raccourcie), décalage, taille. Index à deux niveaux au-delà de 64 Kio d'index. |
| `BLOOM` | Filtre de Bloom par segment (10 bits/clé, ~1 % de faux positifs), découpé par blocs de 4 Kio pour lecture partielle. |
| `EXPIRY` | Plage d'échéances et liste triée des échéances (pour l'expiration active sans parcourir les données). |
| `PROPS` | Nombre d'entrées, de suppressions, taille brute, plage de clés, plage de LSN, statistiques par type. |

Entrée de données :

```
shared_len   : varint   (octets communs avec la clé précédente)
unshared_len : varint
value_len    : varint
key_delta    : octets
header       : type (u8), encodage (u8), version (varint), expire_at (varint, 0 = aucun)
value        : octets (ou référence vers un segment de valeurs volumineuses : segment_id + décalage + taille)
```

Les suppressions sont représentées par une entrée de type `TOMBSTONE` jusqu'au compactage final.

---

## 5. Segment vectoriel

Un segment vectoriel couvre **une collection** dans **une partition**.

| Section | Contenu |
|---|---|
| `IDMAP` | Tableau trié `id externe → position interne` (u64) ; pour les UUID, table séparée de 16 octets par entrée ; et tableau inverse `position → id`. |
| `VERSIONS` | Version (LSN) de chaque point, par position. |
| `VEC_<nom>` | Vecteurs complets du vecteur nommé, contigus, alignés sur 64 octets, type `f32`/`f16`/`u8`. |
| `QVEC_<nom>` | Vecteurs quantifiés : int8 (+ paramètres min/échelle par dimension), binaires (bits empaquetés), ou codes PQ (+ livre de codes). |
| `HNSW_<nom>` | En-tête (m, ef_construct, point d'entrée, niveau max) ; niveau de chaque nœud ; listes d'adjacence du niveau 0 à taille fixe (`2m` voisins, u32) ; niveaux supérieurs en listes compactes. |
| `VAMANA_<nom>` | (Index disque) Blocs de 4 Kio par nœud : vecteur complet + nombre de voisins + voisins ; PQ en section séparée chargée en RAM. |
| `PAYLOAD` | Charge utile par position, en JSON binaire compact (format type MessagePack, crate `rmp-serde`), blocs compressés. |
| `PIDX_<champ>` | Index de charge utile : mots-clés → bitmaps `roaring` ; numériques → paires (valeur, position) triées + index de blocs ; géo → geohash → bitmaps ; texte → dictionnaire + listes de positions. |
| `STATS` | Cardinalités, histogrammes, nombre de points, nombre de supprimés au gel. |
| `LINKS` | Liens point → objet (pour les cascades). |

Le **bitmap de suppressions** n'est pas dans le segment (qui est immuable) : il est conservé dans le manifeste de la partition sous forme de fichier compagnon `<segment_id>.del` (roaring sérialisé), réécrit à chaque point de contrôle.

---

## 6. Segment de métadonnées objet

Format SST (§4) avec clés internes objet (§9) et valeurs protobuf `ObjectMeta` :

```protobuf
message ObjectMeta {
  uint64 size = 1;
  bytes  etag = 2;
  map<string, bytes> checksums = 3;     // "crc32c", "sha256", …
  string content_type = 4;
  map<string, string> headers = 5;      // Content-Encoding, Cache-Control, …
  map<string, string> user_meta = 6;    // x-amz-meta-*
  map<string, string> tags = 7;
  uint64 modified_at = 8;
  bytes  version_id = 9;
  bool   delete_marker = 10;
  StorageClass storage_class = 11;
  oneof data {
    bytes inline = 12;
    ChunkList chunks = 13;
  }
  Retention retention = 14;
  repeated Link links = 15;
  uint32 parts_count = 16;              // pour l'ETag multipart et GetObject?partNumber
}

message ChunkList {
  uint32 chunk_size = 1;
  repeated ChunkRef chunks = 2;
}

message ChunkRef {
  bytes  chunk_id = 1;     // 16 octets
  uint64 offset = 2;       // position dans l'objet
  uint32 length = 3;
  uint32 crc32c = 4;
  BlobLocation location = 5;
}
```

---

## 7. Blocs d'objets

### 7.1 `LocalBlobStore` : fichiers d'extension

```
<data_dir>/blobs/<disque>/ext-<numéro>.blob     (1 Gio, préalloué)
```

- Un bloc occupe une plage alignée sur 4 Kio : `en-tête de bloc (64 o) | données | bourrage`.
- En-tête de bloc : `magic "NDLBLK01"`, `chunk_id`, longueur, CRC32C des données, drapeaux (chiffré, compressé), `key_id`.
- **Table d'allocation** : `chunk_id → (fichier, décalage, longueur)`, stockée dans une SST interne de la partition système locale du nœud, journalisée.
- Récupération d'espace : quand un fichier d'extension a plus de 50 % d'espace libéré, ses blocs vivants sont recopiés dans un nouveau fichier (classe `background`), puis le fichier est libéré.

### 7.2 `ExternalBlobStore` : objets et paquets

- Blocs ≥ 4 Mio : un objet externe par bloc, clé `<préfixe>/chunks/<2 premiers octets hex>/<chunk_id hex>`.
- Blocs < 4 Mio : regroupés dans des **paquets** de 64 Mio maximum, clé `<préfixe>/packs/<pack_id>` ; le paquet contient un index en fin de fichier ; la référence du bloc contient `(pack_id, décalage, longueur)` et la lecture utilise une requête par plage.
- Les segments et manifestes sont stockés sous `<préfixe>/partitions/<partition_id>/…`.

### 7.3 `DistributedBlobStore` : fragments

- Fragment = `en-tête (magic "NDLFRG01", chunk_id, index du fragment, k, m, longueur du bloc d'origine, CRC du fragment) | données`.
- Les fragments sont stockés sur les nœuds `storage` dans des fichiers d'extension (même format que §7.1).
- Inventaire par nœud : SST locale `chunk_id → index de fragment, emplacement`.

---

## 8. Manifeste de partition

Fichier : `<data_dir>/partitions/<partition_id>/MANIFEST-<numéro>` + fichier pointeur `CURRENT` (contenant le nom du manifeste courant), mis à jour par renommage atomique. En mode désagrégé, copie dans le stockage objet.

```protobuf
message PartitionManifest {
  uint32 format = 1;
  uint32 partition_id = 2;
  uint64 manifest_no = 3;
  uint64 checkpoint_lsn = 4;          // reprise du journal à partir de ce LSN
  uint64 applied_lsn_at_write = 5;
  repeated NamespaceRange namespaces = 6;
  repeated SegmentEntry segments = 7;
  repeated DeleteBitmap delete_bitmaps = 8;
  RaftState raft = 9;                 // dernier index/terme inclus dans le point de contrôle, membres
  repeated StagedObject staged = 10;  // objets provisoires en cours (si non stockés en segment)
  uint64 created_at = 11;
}

message SegmentEntry {
  bytes  segment_id = 1;
  SegmentKind kind = 2;
  uint32 collection_id = 3;           // pour les segments vectoriels
  uint32 level = 4;                   // niveau LSM pour les SST
  bytes  min_key = 5;
  bytes  max_key = 6;
  uint64 min_lsn = 7;
  uint64 max_lsn = 8;
  uint64 size_bytes = 9;
  Location location = 10;             // local, objet (clé), ou les deux
  Tier pinned_tier = 11;
}
```

---

## 9. Encodage des clés internes

Toutes les entrées ordonnées d'une partition partagent un espace de clés internes, trié par octets :

```
┌──────┬─────────────────┬────────────────────────────────────────────┐
│ type │ namespace_id    │ suite spécifique                           │
│ (1)  │ (4, gros-bout.) │                                            │
└──────┴─────────────────┴────────────────────────────────────────────┘
```

| Type | Préfixe | Suite |
|---|---|---|
| `0x01` | Clé-valeur | `db (1)` · `clé utilisateur (échappée)` |
| `0x02` | Échéances | `db (1)` · `expire_at (8, gros-boutiste)` · `clé` |
| `0x10` | Métadonnées objet | `bucket_id (4)` · `clé objet (échappée)` · `0x00 0x00` · `version inversée (8)` |
| `0x11` | Multipart en cours | `bucket_id` · `clé` · `upload_id` |
| `0x12` | Objets provisoires | `staged_id (16)` |
| `0x20` | Charge utile vectorielle (segment mutable) | `collection_id (4)` · `point_id (8 ou 16)` |
| `0x30` | Index inverse des liens | `bucket_id` · `clé objet` · `modèle (1)` · `référence` |
| `0x7F` | Système | Données internes de la partition |

**Échappement** des clés utilisateur pour préserver l'ordre et permettre les suffixes : `0x00` → `0x00 0xFF`, terminaison `0x00 0x00`. Les entiers en **gros-boutiste** dans les clés pour que l'ordre des octets corresponde à l'ordre numérique. La version inversée (`u64::MAX - version`) place la version la plus récente en premier.

---

## 10. Identifiants

Les identifiants composés de 16 octets sont encodés en **gros-boutiste** : comparer leurs octets (noms de fichiers, clés triées, chaîne base32) revient à comparer leurs champs dans l'ordre. Implémentation : `ndaal-common::ids`.

| Identifiant | Format | Génération |
|---|---|---|
| `node_id` | UUID v4 | Au premier démarrage, stocké dans `<data_dir>/NODE_ID` |
| `cluster_id` | UUID v4 | À l'initialisation du cluster |
| `namespace_id` | u32 | Attribué par le pilote de placement (séquence) |
| `partition_id` | u32 | Attribué par le pilote de placement |
| `collection_id`, `bucket_id` | u32 | Attribués par le pilote de placement |
| `lsn` | u64 | Séquence par partition, commence à 1 |
| `segment_id` | 16 octets : `partition_id (4) · lsn (8) · compteur (4)` | Par la partition qui le crée |
| `chunk_id` | 16 octets : `partition_id (4) · lsn de création (8) · compteur (4)` | Par la partition ; unique sans coordination |
| `version_id` objet | 16 octets, affiché en base32 Crockford (26 caractères) : `horodatage (8) · lsn (8)` | Ordre chronologique |
| `staged_id` | Préfixe `stg_` + ULID | Par la partition |
| `tx_id` | u128 : `partition_id · lsn` | À la validation |
| `upload_id` multipart | Préfixe `mpu_` + 26 caractères base32 | Par la partition |
| Point vectoriel | u64 ou UUID (fourni par le client) | Client |

---

## 11. Catalogue

Stocké dans le groupe Raft système, en protobuf :

```protobuf
message Namespace {
  uint32 id = 1;
  string name = 2;                    // [a-z0-9-]{1,63}
  NamespaceMode mode = 3;             // SINGLE, SHARDED
  repeated uint32 partitions = 4;
  Durability default_durability = 5;
  Quotas quotas = 6;
  repeated InvalidationRule invalidation = 7;
  repeated PlacementRule placement = 8;
  bytes wrapped_data_key = 9;         // clé de données chiffrée par la clé maîtresse
  uint64 created_at = 10;
  NamespaceState state = 11;          // ACTIVE, DELETING
}

message Collection {
  uint32 id = 1;
  uint32 namespace_id = 2;
  string name = 3;
  repeated VectorConfig vectors = 4;  // nom, dimension, métrique, type, index, quantification
  repeated PayloadIndex payload_indexes = 5;
  OptimizerConfig optimizer = 6;      // seuils de gel, de compactage, de full_scan
  uint64 created_at = 7;
}

message Bucket {
  uint32 id = 1;
  uint32 namespace_id = 2;
  string name = 3;                    // règles de nommage S3
  bool versioning = 4;
  repeated LifecycleRule lifecycle = 5;
  bytes policy_json = 6;
  repeated CorsRule cors = 7;
  ObjectLockConfig object_lock = 8;
  bool cascade_delete = 9;
  bool dedup = 10;
}

message User {
  string name = 1;
  repeated Credential credentials = 2; // hachés (Argon2id) ou chiffrés (S3)
  repeated string roles = 3;
  string default_namespace = 4;
  bool disabled = 5;
}

message Role {
  string name = 1;
  repeated Permission permissions = 2;
}

message Permission {
  string namespace = 1;               // "*" autorisé
  Model model = 2;                    // KV, VECTOR, OBJECT, ADMIN, ALL
  string resource = 3;                // préfixe de clé, collection, compartiment/préfixe ; "*" autorisé
  repeated Action actions = 4;        // READ, WRITE, DELETE, LIST, ADMIN
}
```

---

## 12. Chiffrement

- **Clé maîtresse** (KEK) : fournie par un fournisseur (`file`, `env`, `kms`). Jamais écrite en clair sur disque par Ndaal.
- **Clés de données** (DEK) : une par espace de noms (+ une pour les données système), AES-256, générées aléatoirement, stockées **enveloppées** (AES-256-GCM avec la KEK) dans le catalogue.
- Chaque bloc chiffré : `nonce 96 bits aléatoire || chiffré || tag 128 bits` ; données associées (AAD) = `segment_id || numéro de bloc` (ou `chunk_id`) pour empêcher le déplacement de blocs.
- Rotation de KEK : ré-enveloppement des DEK uniquement. Rotation de DEK : nouvelles écritures avec la nouvelle clé (`key_id` dans les en-têtes), réécriture progressive par compactage.

---

## 13. Arborescence sur disque

```
<data_dir>/
├── NODE_ID
├── CLUSTER_ID
├── LOCK                         # verrou exclusif (flock) : un seul processus
├── config.effective.toml        # configuration effective au dernier démarrage (diagnostic)
├── system/                      # partition système locale (table d'allocation, inventaire)
├── control/                     # journal et instantanés du groupe Raft système (si rôle control)
├── wal/
│   └── shard-NN/*.wal
├── partitions/
│   └── <partition_id>/
│       ├── CURRENT
│       ├── MANIFEST-*
│       ├── segments/*.seg
│       └── segments/*.del
├── blobs/
│   └── <disque>/ext-*.blob
├── cache/                       # partie NVMe du cache hybride
└── tmp/                         # fichiers temporaires (nettoyés au démarrage)
```

---

## 14. Politique de compatibilité

| Élément | Règle |
|---|---|
| Lecture des anciens formats | Garantie pour les formats des deux publications majeures précédentes |
| Écriture | Toujours au format courant, sauf si un **drapeau de fonctionnalité de cluster** n'est pas encore activé (cluster en cours de mise à jour) |
| Migration | Paresseuse, par compactage ; une commande `ndaal admin upgrade-formats` force la réécriture |
| Retour arrière | Possible tant que le drapeau de fonctionnalité du nouveau format n'est pas activé |
| Tests | Un corpus de fichiers produits par chaque format publié est conservé dans `tests/format-corpus/` et relu en CI |
