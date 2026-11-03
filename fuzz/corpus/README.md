# Corpus de fuzzing versionné

Un dossier par cible de `fuzz/fuzz_targets/` (même nom). Chaque fichier est une entrée brute : ses octets sont la donnée de test (pas de conversion de fins de ligne, voir `.gitattributes`). Le corpus est rejoué en Rust stable à chaque demande de fusion par `crates/ndaal-fuzz/tests/corpus.rs`, et sert de point de départ au fuzzing nocturne (`.github/workflows/fuzz.yml`).

## `common_codec`

| Fichier | Contenu |
|---|---|
| `varint-300` | varint de 300 sur 2 octets (`AC 02`) |
| `varint-u64-max` | varint de `u64::MAX` sur 10 octets |
| `u64-le` | entier de 64 bits petit-boutiste |
| `bytes-hello` | octets préfixés par leur longueur : `hello` |
| `str-accents` | chaîne UTF-8 préfixée, avec accents |
| `crockford-upper` | identifiant base32 Crockford canonique |
| `crockford-lower-aliases` | le même en minuscules, avec l'alias `l` pour `1` |

## `segment_reader`

| Fichier | Contenu |
|---|---|
| `valid-two-sections` | segment valide de deux sections (trois blocs, dont un LZ4) |
| `torn-footer` | le même, tronqué de 100 octets (pied incomplet) |
| `lz4-block` | octet de compression `1` puis un bloc LZ4 sans sa fin (la cible le scelle) |

## `wal_reader`

| Fichier | Contenu |
|---|---|
| `valid-three-records` | fichier de journal valide (en-tête et trois trames) |
| `torn-last-record` | le même, avec un octet abîmé dans la dernière trame |
| `frames-without-header` | deux trames sans en-tête de fichier (la cible en ajoute un) |

Pour réintégrer le corpus d'une nuit : télécharger l'artefact, puis `cargo fuzz cmin <cible>` et une demande de fusion.
