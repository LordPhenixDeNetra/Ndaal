# J02-C1 — Format et lecteur du journal : spécification

**Tâches :** T02.07, T02.10 ([CAHIER_DES_TACHES.md](../../../CAHIER_DES_TACHES.md#j02--noyau-de-stockage)).

**Exigences :** ENF-DUR-01, ENF-DUR-03, ENF-QUAL-03, ENF-COMPAT-04.

**Sources :**
- [FORMATS §1](../../../FORMATS_DE_DONNEES.md#1-conventions) et [§2](../../../FORMATS_DE_DONNEES.md#2-journal-wal) ;
- [ARCHITECTURE §6](../../../ARCHITECTURE.md#6-journal-unifié-wal) ;
- [DECISIONS](../../../DECISIONS.md) ADR-004 et ADR-007.

**Statut :** conception validée par le responsable du projet le 2026-10-28, section par section.

## 1. Contexte et périmètre

J02-C (le journal) a été découpé en deux, avec l'accord du responsable :
- **C1 (ce document) :** le format et le lecteur. Une partie pure, sans aucune E/S.
- **C2 :** l'écrivain à écriture groupée, les fichiers préalloués et recyclés, et les modes de durabilité (T02.08, T02.09, T02.11).

**Hors du périmètre :**
- tout ce qui relève de C2 ;
- l'encodage des effets (FORMATS §2.5 et §2.6), qui viendra avec les moteurs : pour le journal, la charge utile est opaque ;
- la compression et le chiffrement des enregistrements : leurs drapeaux existent dans le format, mais rien ne les écrit encore.

## 2. Décisions

| # | Décision | Raison |
|---|---|---|
| 1 | Le lecteur travaille sur le **contenu entier d'un fichier en mémoire** ; le recollage des fragments est un composant à part, qui garde son état d'un fichier au suivant | Détecter une corruption exige de regarder au-delà de la trame invalide. C1 reste pur et entièrement testable. Coût : 64 Mio de mémoire par shard pendant la reprise |
| 2 | **Fragments de 1 Mio** au lieu de 16 Mio | Ils bornent la taille d'un vidage, donc la zone qu'une coupure peut déchirer (décision 3). Coût : 40 octets d'en-tête par Mio, soit 0,004 % |
| 3 | **Zone déchirée bornée** : l'écrivain n'a qu'un vidage en cours, d'au plus `MAX_FLUSH` = 2 Mio, et n'acquitte qu'après son `fdatasync`. Le lecteur ne conclut à une corruption que si une trame valide se trouve **au moins `MAX_FLUSH` octets** après la trame invalide | Les pages d'une écriture peuvent atteindre le disque dans le désordre lors d'une coupure. Sans cette borne, une coupure ordinaire serait prise pour une corruption. Coût : une vraie corruption dans les 2 derniers Mio d'un fichier est prise pour une fin de journal (nouvel ADR-033) |
| 4 | Une trame valide qui porte un drapeau « compressé » ou « chiffré » est refusée (`Unsupported`), et non prise pour une corruption | Jamais lue de travers ; la prise en charge viendra avec ces fonctions |

## 3. Composants (crate `ndaal-wal`, aucune E/S)

| Module | Rôle |
|---|---|
| `crc` | CRC32C masqué comme dans LevelDB : `((crc >> 15) \| (crc << 17)) + 0xa282ead8`. Dépendance `crc32c`, déjà nommée par FORMATS §1 |
| `header` | `FileHeader { shard, file_no, node: NodeId, created_at, recycle_epoch, flags }` : encodage et décodage du bloc de 4 096 octets. Magic `NDLWAL01`, format 1, CRC des octets 0 à 4091 placée aux octets 4092 à 4095 |
| `record` | `RecordType` (codes de FORMATS §2.4), `Record { kind, partition: PartitionId, lsn: Lsn, term, timestamp, payload }`, et `encode(record, epoch, out)`, qui écrit une ou plusieurs trames de 40 octets d'en-tête et de 1 Mio de charge utile au plus |
| `reader` | `FileReader::new(octets)` vérifie l'en-tête, puis rend des `Frame` dont la charge utile est empruntée (sans copie). Il termine par `End { valid_len }` ou `Corrupt { offset }` |
| `reassemble` | `Reassembler` recolle les fragments consécutifs, même quand ils passent d'un fichier au suivant. Il rend un `Record` dont la charge utile est empruntée pour une trame entière et copiée pour un enregistrement fragmenté (`Cow`) |

**Erreurs.** Elles sont typées avec `thiserror` : en-tête invalide, version de format inconnue, drapeau non pris en charge, fragment manquant, en double ou hors d'ordre. Aucune `panic!` sur des octets lus.

**Identifiants.** `PartitionId`, `Lsn` et `NodeId` viennent de `ndaal-common`.

## 4. Règles de lecture

1. **L'en-tête de fichier** doit avoir le bon magic, une version connue (1) et une CRC juste ; sinon, c'est une erreur. Le drapeau « chiffré » donne `Unsupported`.
2. **Une trame est valide** si toutes ces conditions tiennent :
   - au moins 40 octets restent ;
   - `len` ≤ 1 Mio et la trame tient dans le fichier ;
   - la CRC est juste ;
   - les 16 bits d'époque sont égaux à ceux de `recycle_epoch` de l'en-tête ;
   - le type est connu ;
   - les bits de drapeau réservés (4 à 7) sont nuls.

   Une trame `PADDING` est sautée.
3. **À la première trame invalide**, à l'octet `o`, le lecteur cherche une trame valide à partir de `o + MAX_FLUSH` jusqu'à la fin du fichier. Il teste d'abord les conditions bon marché (longueur, époque, type, drapeaux), puis la CRC. S'il en trouve une, il rend `Corrupt { offset: o }` ; sinon `End { valid_len: o }`. Ce qui est entre `o` et `o + MAX_FLUSH` est abandonné.
4. **Des zéros** (fichier préalloué) et **d'anciens enregistrements** d'un fichier recyclé (mauvaise époque) sont des trames invalides, donc une fin de journal.
5. **Les fragments** portent les bits « premier » et « dernier » ; une trame entière porte les deux. Un fragment suivant doit avoir la même partition, le même LSN, le même type, le même terme et le même horodatage. Sinon, c'est une erreur de recollage.

## 5. Tests et critères de fin

| Critère | Preuve |
|---|---|
| T02.07 : tests de propriétés aller-retour | `proptest` : enregistrements de tous les types, charges utiles de 0 octet à plus de 3 Mio, donc fragmentées, époques quelconques. La relecture rend exactement les enregistrements écrits |
| T02.07 : tout octet modifié est détecté | Sur un petit fichier, chaque octet puis chaque bit est inversé tour à tour. Le lecteur ne rend jamais un enregistrement altéré : il s'arrête (`End`), signale `Corrupt`, ou renvoie une erreur |
| T02.10 : une troncature à n'importe quel octet donne un préfixe valide | Pour chaque longueur de coupe, la relecture rend un préfixe exact des enregistrements, puis `End` |
| T02.10 : une corruption au milieu est signalée | Une trame abîmée suivie de trames valides au-delà de 2 Mio donne `Corrupt`. Une zone déchirée de moins de 2 Mio, avec des trames valides derrière, donne `End` |
| Fragments | Recollage entre deux fichiers ; un fragment manquant, en double ou hors d'ordre est signalé |
| Robustesse | Cible de fuzzing `wal_reader`, avec son corpus |
| Compatibilité | Un fichier de journal de référence en version 1, versionné dans les tests, se relit toujours à l'identique |
| Tendance de performance | Banc `criterion` `wal/encode_decode` sur de petits enregistrements. Le débit de 500 000 enregistrements par seconde est un critère de l'écrivain (C2) |

## 6. Documentation modifiée avec C1

- **FORMATS §2.3** : fragments de 1 Mio, règle de la zone déchirée, drapeaux non pris en charge refusés.
- **DECISIONS** : nouvel ADR-033, « Zone déchirée bornée du journal ».
- **CAHIER_DES_TACHES** : critère de fin de T02.08 complété par « un seul vidage en cours, de 2 Mio au plus, acquittement après `fdatasync` ». Cette modification a reçu l'accord du responsable le 2026-10-28.

## 7. Risques

| Risque | Parade |
|---|---|
| Corruption réelle dans les 2 derniers Mio d'un fichier, prise pour une fin de journal | Borne la plus petite possible (décision 2) ; ADR-033 ; en cluster, les réplicas couvrent cette perte (ENF-DUR-02) |
| Une charge utile malveillante contenant une trame valide, après une coupure | La recherche commence 2 Mio plus loin ; l'époque de recyclage et le CRC masqué rendent une collision fortuite improbable. À revoir avec le chiffrement |
| Époque sur 16 bits dans la trame : après 65 536 recyclages, une ancienne trame pourrait sembler valide | Le recyclage (C2) ne réutilise jamais un fichier dont l'époque basse retomberait sur une valeur encore lisible ; à préciser en C2 |
| Mémoire de 64 Mio par shard pendant la reprise | Acceptable ; à revoir si les bancs de reprise (J07) le demandent |
