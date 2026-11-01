# J02-C2 — Écrivain du journal, fichiers et modes de durabilité : spécification

**Tâches :** T02.08, T02.09, T02.11 ([CAHIER_DES_TACHES.md](../../../CAHIER_DES_TACHES.md#j02--noyau-de-stockage)).

**Exigences :** ENF-DUR-01, ENF-DUR-03, ENF-DUR-04, ENF-PERF-03, ENF-QUAL-03, P8, ADR-003, ADR-004, ADR-016, ADR-017, ADR-033.

**Sources :**
- [ARCHITECTURE §6](../../../ARCHITECTURE.md#6-journal-unifié-wal) ;
- [FORMATS §2](../../../FORMATS_DE_DONNEES.md#2-journal-wal) ;
- [EXPLOITATION §3](../../../EXPLOITATION.md#3-référence-de-configuration) ;
- la spécification de J02-C1 ([2026-10-28-j02c1-wal-format-design.md](2026-10-28-j02c1-wal-format-design.md)) ;
- POINTS_REPORTES P-59, P-60, P-61.

**Statut :** conception validée par le responsable du projet le 2026-11-01, section par section.

## 1. Contexte et périmètre

C1 a livré le format et le lecteur. C2 livre :
- l'écrivain à écriture groupée ;
- les fichiers préalloués et recyclés ;
- les modes de durabilité ;
- deux changements de format, décidés avant que des fichiers n'existent en production.

**Hors du périmètre :**
- le rejeu des enregistrements dans les partitions (T02.18) ;
- l'appel de `release` par les points de contrôle (T02.19) ;
- le mode `replicated` (J09) ;
- `O_DIRECT` et `io_uring` (J07) ;
- les modes par défaut selon le modèle (`durability.kv_default`, `durability.other_default`), qui viendront avec les espaces de noms.

## 2. Décisions

| # | Décision | Raison | Coût |
|---|---|---|---|
| 1 | **CRC propre à l'en-tête de trame** : l'en-tête passe de 40 à 44 octets (P-60) | Le lecteur rejette un faux candidat en 44 octets : au pire, la recherche après une trame abîmée tombe d'environ 90 Go à environ 46 Mo de CRC | 4 octets par trame |
| 2 | **CRC ensemencées** par l'époque de recyclage sur 64 bits et le `file_no` de l'en-tête du fichier (P-59) | Une trame d'une autre vie du fichier, ou recopiée d'un autre fichier, échoue à la CRC, quelle que soit l'époque basse | 16 octets de plus dans chaque calcul de CRC |
| 3 | **Un écrivain par shard, sur le cœur du shard**, avec une tâche de vidage ; l'écriture et le `fdatasync` forment un seul travail `Critical` du pool d'E/S | Réutilise l'ordonnanceur et ses classes ; aucun thread de plus ; testable en simulation (`DeviceDisk`) | — |
| 4 | **À la reprise, effacement de `o` à `o + MAX_FLUSH`** avant d'écrire, `o` étant la fin du journal trouvée par le lecteur | Sans cela, des trames non acquittées d'un vidage déchiré pourraient ressusciter derrière un nouvel enregistrement de même longueur | Au plus 2 Mio de zéros au démarrage |
| 5 | **La fin d'un fichier est toujours remplie** par un enregistrement `PADDING` ; l'écrivain ne laisse jamais entre 1 et 43 octets libres | Un fichier qui n'est pas le dernier se lit toujours jusqu'à sa fin exacte : la reprise peut traiter une fin prématurée comme une corruption (P-61) | Quelques octets par fichier |
| 6 | **T02.08 n'est cochée qu'après la mesure de débit sur la machine de référence** (D-05) | Le critère vise cette machine ; même traitement que T02.03 (P-53) | La tâche reste ouverte malgré le code fini |

Les décisions 1 et 2 font l'objet d'un nouvel **ADR-034**, « CRC d'en-tête et CRC ensemencées du journal ». La version de format reste **1** : aucun fichier de journal n'existe hors des tests ; le fichier de compatibilité et le corpus de fuzzing sont régénérés une seule fois.

## 3. Format de trame (FORMATS §2.3, nouvelle disposition)

| Décalage | Taille | Champ |
|---|---|---|
| 0 | 4 | `crc` : CRC32C masqué et ensemencé de tout ce qui suit (octets 4 jusqu'à la fin de la charge utile) |
| 4 | 4 | `header_crc` : CRC32C masqué et ensemencé des octets 8 à 43 |
| 8 | 4 | `len` |
| 12 | 1 | `type` |
| 13 | 1 | `flags` |
| 14 | 2 | `recycle_epoch` (16 bits de poids faible, filtre rapide) |
| 16 | 4 | `partition_id` |
| 20 | 8 | `lsn` |
| 28 | 8 | `term` |
| 36 | 8 | `timestamp` |
| 44 | `len` | charge utile |

**La graine** est formée de 16 octets placés devant les données couvertes : `recycle_epoch` (u64 petit-boutiste) suivi de `file_no` (u64 petit-boutiste). Le masque reste celui de LevelDB.

**Ordre de validation d'une trame :** place restante, `len`, époque basse, type, drapeaux, `header_crc`, puis `crc`.

`encode` reçoit désormais l'époque et le `file_no`. `FRAME_HEADER_LEN` vaut 44 ; `MAX_FRAGMENT` (1 Mio) et `MAX_FLUSH` (2 Mio) ne changent pas.

## 4. Écrivain (T02.08)

### 4.1 Interface (crate `ndaal-wal`, module `writer`)

- `WalWriter::open(disk, dir, shard, node, config, clock) -> Result<WalWriter, WalError>`, générique sur `D: Disk`. En réel, `disk` est un `PoolDisk` de classe `Critical` ; en simulation, un `DeviceDisk`.
- `append(&self, record: &Record) -> Result<Position, Busy>` est synchrone et n'attend jamais. Il encode dans le tampon actif. Au-delà de la borne du tampon en attente (2 × `MAX_FLUSH` non vidés), la réponse est `Busy` (P8).
- `durable(&self, position) -> impl Future<Output = Result<(), WalError>>` se termine quand le `fdatasync` qui couvre la position a réussi.
- `durable_position(&self) -> Position`.
- `submit(&self, record, mode: Durability)`, pour les modes (§6).
- `release(&self, file_no)`, pour le recyclage (§5).

`Position` est la position d'un enregistrement dans le flux du shard : un numéro de fichier et un décalage, ordonnés.

### 4.2 Tâche de vidage

- **Un seul vidage en cours.** Pendant ce vidage, les ajouts remplissent l'autre tampon (double tampon).
- **Déclenchement**, dès qu'aucun vidage n'est en cours et que le tampon n'est pas vide :
  - immédiatement si le tampon atteint `durability.group_commit_max_bytes` (256 Kio) ;
  - sinon après avoir laissé passer une fois les autres tâches prêtes du cœur ;
  - au plus tard `durability.group_commit_max_delay` (200 µs) après le premier enregistrement non vidé.
- **Étendue d'un vidage :**
  - au plus `MAX_FLUSH` octets, page partielle réécrite comprise, coupés sur une frontière de trame ;
  - l'écriture commence à la page de 4 Kio qui contient la fin précédente et finit par des zéros jusqu'à la page suivante (FORMATS §2.3).
- **Exécution :** un seul travail `Critical`, `write_all_at` puis `sync_data`. Le repère de durabilité avance, et les attentes couvertes se réveillent.
- **Échec d'écriture ou de `sync_data` :**
  - l'écrivain passe en état `Failed` : les attentes reçoivent l'erreur et tout nouvel `append` est refusé ;
  - aucun `fdatasync` n'est réessayé, car après un échec, Linux marque les pages propres. Le nœud repart du journal.

### 4.3 Métriques (EXPLOITATION §8)

| Nom | Type |
|---|---|
| `ndaal_wal_fsync_seconds{shard}` | histogramme |
| `ndaal_wal_durable_lsn{shard}` | jauge (position durable) |
| `ndaal_wal_flush_bytes{shard}` | histogramme |
| `ndaal_wal_append_busy_total{shard}` | compteur |

## 5. Fichiers et recyclage (T02.09)

- **Chemin :** `<dir>/shard-NN/<numéro sur 16 chiffres hexadécimaux>.wal` ; **taille :** `wal.file_size` (64 Mio), réservée par `File::allocate`.
- **Fichier suivant préparé à l'avance** par la tâche de vidage, dès que le fichier courant est à moitié plein : réservation, en-tête, `sync_data`, `sync_dir`. Le passage d'un fichier au suivant ne coûte rien sur le chemin des écritures.
- **Une trame n'est jamais coupée entre deux fichiers** ; un enregistrement fragmenté peut l'être. La fin d'un fichier est remplie par un `PADDING` (décision 5).
- **Recyclage :**
  - `release(file_no)` renomme le fichier en `free-<numéro>.wal`, puis `sync_dir`. Au démarrage, les fichiers `free-*` forment la liste des fichiers libres ; seuls les fichiers numérotés sont du journal ;
  - un nouveau fichier prend d'abord un fichier libre : en-tête réécrit (nouveau numéro, `recycle_epoch` + 1, drapeau « recyclé »), `sync_data`, **puis** renommage vers le nouveau numéro et `sync_dir`. Un arrêt au milieu laisse un fichier `free-*` à l'en-tête quelconque, qui sera réécrit, jamais un fichier numéroté invalide ;
  - sinon, le fichier est créé et réservé.
- **Reprise (`open`) :**
  1. trouver le dernier fichier ;
  2. le lire avec `FileReader` jusqu'à la fin `o` ;
  3. effacer de `o` à `min(o + MAX_FLUSH, taille)`, puis `sync_data` ;
  4. reprendre l'écriture en `o`.

  Un `Corrupt` ou une erreur d'en-tête est rendu à l'appelant : l'écrivain ne s'ouvre pas sur un journal corrompu.

## 6. Modes de durabilité (T02.11)

`pub enum Durability { None, Async, Sync }` et `submit(record, mode)` :

| Mode | Comportement | Perte autorisée en cas de coupure |
|---|---|---|
| `None` | Rien n'est journalisé ; réponse immédiate | Tout ce qui n'est pas dans un point de contrôle |
| `Async` | `append`, réponse immédiate ; le vidage a lieu au plus tard après `durability.async_flush_interval` (100 ms) | Les enregistrements des 100 dernières millisecondes au plus |
| `Sync` | `append`, puis réponse après `durable(position)` | Aucune |

## 7. Configuration (EXPLOITATION §3)

| Clé | Type | Défaut |
|---|---|---|
| `wal.file_size` | taille (nouveau type `Kind::Size` : `64MiB`, `1GiB`…), de 4 Mio à 1 Gio | `64MiB` |
| `durability.async_flush_interval` | durée | `100ms` |
| `durability.group_commit_max_delay` | durée | `200us` |
| `durability.group_commit_max_bytes` | taille, de 4 Kio à 2 Mio | `256KiB` |

Le type `Kind::Size` accepte un entier suivi de `B`, `KiB`, `MiB` ou `GiB`. Ses messages sont traduits dans la CLI (`cli-expected-size`).

## 8. Tests et critères de fin

| Critère | Preuve |
|---|---|
| T02.08 : le shard n'est jamais bloqué par un `fdatasync` | Simulation avec un `SimDevice` lent : pendant un vidage, `append` répond sans attendre et une autre tâche du cœur avance |
| T02.08 : un vidage en cours, ≤ `MAX_FLUSH`, fin sur une frontière de trame, acquittement après `fdatasync` | `proptest` avec un disque qui enregistre chaque écriture et chaque `sync_data` |
| T02.08 : débit ≥ 500 000 petits enregistrements/s par shard | Banc `wal/append` ; mesure locale consignée. Cochage après la mesure sur la machine de référence (décision 6) |
| T02.09 : un fichier recyclé ne fait jamais relire un ancien enregistrement | Test dédié : fichier recyclé encore plein d'anciennes trames valides de l'époque précédente, et trame recopiée d'un autre fichier : aucune n'est relue |
| T02.11 : seules les pertes autorisées par chaque mode | Scénario `wal_crash` dans `ndaal-sim` : écritures dans les trois modes, coupure à un instant tiré de la graine (`SimDisk`, pages perdues ou déchirées), reprise et vérification ; ajouté à `sweep` (10 000 coupures par nuit, ENF-DUR-01) |
| Reprise | Un vidage déchiré, suivi d'une reprise et d'un enregistrement de même longueur au même endroit, ne ressuscite aucune trame non acquittée |
| Format | Tests de C1 adaptés à 44 octets ; trames d'une autre époque ou d'un autre `file_no` refusées ; fichier de compatibilité et corpus régénérés |
| Configuration | Tests de `Kind::Size` (valeurs limites, unités, refus) et des nouvelles clés |

## 9. Documentation modifiée avec C2

- **FORMATS §2.3** : en-tête de 44 octets, CRC d'en-tête, graine ; §2.1 : remplissage de fin de fichier.
- **DECISIONS** : ADR-034.
- **ARCHITECTURE §6.3** : implémentation de l'écriture groupée et de la reprise de l'écrivain.
- **EXPLOITATION** : §3 pour les nouvelles clés ; §8 pour les métriques `ndaal_wal_*`.
- **POINTS_REPORTES** : P-59 et P-60 fermés ; le reste selon la revue finale.

## 10. Risques

| Risque | Parade |
|---|---|
| Débit insuffisant à cause d'un passage par le pool par vidage | Un seul travail par vidage ; banc `wal/append` ; `io_uring` en J07 |
| Comportement de `fdatasync` après erreur | État `Failed` définitif, jamais de réessai (décision documentée) |
| Arrêt au milieu d'un recyclage | L'en-tête est réécrit et synchronisé **avant** le renommage vers le nouveau numéro (§5) : un fichier numéroté a toujours un en-tête valide |
| La simulation ne déchire pas les pages comme le matériel | `SimDisk` perd ou déchire des secteurs dans le désordre ; les bancs de coupure réels viendront en J07 |
