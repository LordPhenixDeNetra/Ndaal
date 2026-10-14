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

Pour réintégrer le corpus d'une nuit : télécharger l'artefact, puis `cargo fuzz cmin <cible>` et une demande de fusion.
