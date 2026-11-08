# T02.14 — Encodage des clés internes : plan d'implémentation

> **Pour les agents :** sous-compétence requise : superpowers:executing-plans, tâche par tâche.
>
> **Pas d'étape de commit** : le responsable commite sur la branche `t0214-keys`, puis ouvre la demande de fusion avec `gh pr create`. Le compte rendu donne les commandes git, et la description de la demande de fusion est écrite dans `.git/pr-body.md`.

**Objectif :** les primitives de clés internes de FORMATS §9 (préfixe, échappement, entiers gros-boutistes, version inversée), avec la preuve que l'ordre des octets est l'ordre logique.

**Architecture :** un fichier `crates/ndaal-common/src/keys.rs`, dans le style de `bytes.rs`. Aucune nouvelle dépendance ; `proptest` est déjà en dépendance de test.

**Spécification :** [docs/superpowers/specs/2026-11-08-t0214-internal-keys-design.md](../specs/2026-11-08-t0214-internal-keys-design.md).

## Contraintes globales

- Aucune panique sur une entrée hostile ; un décodeur n'avance l'entrée qu'en cas de succès.
- ASCII seulement dans les `.rs` ; fichier de 400 lignes au plus, tests compris si possible (sinon tests dans `crates/ndaal-common/tests/keys.rs`).
- Test d'abord, le voir échouer ; mutation quand un test passe du premier coup.
- Toujours `export CARGO_TARGET_DIR="$PWD/target"` avant cargo.

## Points à surveiller en revue

1. **Une clé contenant `0x00 0x00` ou finissant par `0x00`** : aller-retour exact, et ordre respecté contre la même clé sans ce suffixe.
2. **La clé vide** : elle s'encode `0x00 0x00` et se trie avant toute autre clé.
3. **`0x00 0xFF` suivi d'une fin d'entrée** : `DecodeError`, pas une clé acceptée.
4. **Un `get_escaped` sur un préfixe d'une clé échappée** : erreur, entrée intacte.

---

### Tâche 1 : primitives et propriété d'ordre

**Fichiers :** créer `crates/ndaal-common/src/keys.rs` ; modifier `crates/ndaal-common/src/lib.rs` (`pub mod keys;`) ; tests dans `crates/ndaal-common/tests/keys.rs`.

**Interfaces (produit) :** spécification §3.

- [ ] **Tests d'abord** :
  - `escape_round_trip` sur `b""`, `b"a"`, `b"a\0"`, `b"\0\0"`, `b"\0\xff"`, avec un suffixe `b"rest"` laissé dans l'entrée ;
  - `plain_key_is_borrowed` : `get_escaped` sur l'échappement de `b"abc"` rend `Cow::Borrowed` ;
  - `invalid_escapes_are_refused` : `[0x00, 0x01]`, `[0x61]` (sans terminaison), `[0x00]`, `[0x00, 0xff]`, `[0x61, 0x00, 0xff, 0x00]` rendent `Err` et laissent l'entrée intacte ;
  - `prefix_round_trip_and_unknown_type` : aller-retour pour les 8 types ; l'octet `0x03` rend `Err` avec l'entrée intacte ; une entrée de 4 octets rend `Err` ;
  - `be_round_trip` pour `u32` et `u64` ; `inverted(0) == u64::MAX` ;
  - propriétés (`proptest`) : `escaped_order_is_key_order`, `suffix_keeps_order`, `be_order_is_numeric_order`, `inverted_reverses_order`, `composed_key_order_is_tuple_order` (n-uplet `(type, namespace, db, key, expire_at)`, comparé au n-uplet Rust avec `KeyType` par sa valeur d'octet).
- [ ] `cargo test -p ndaal-common --test keys` réussit.

### Tâche 2 : fuzzing, documents, vérifications

**Fichiers :** `crates/ndaal-fuzz/src/lib.rs` (corps `common_codec`) ; `fuzz/corpus/common_codec/` (deux entrées) et `fuzz/corpus/README.md` ; `FORMATS_DE_DONNEES.md` §9.

- [ ] `common_codec` : toute clé acceptée par `get_escaped` se ré-encode exactement en les octets consommés (même aide `round_trip` que les autres décodeurs) ; entrées `escaped-with-zero` (`61 00 ff 62 00 00`) et `escaped-bad-escape` (`61 00 01`) ; `cargo test -p ndaal-fuzz` réussit.
- [ ] FORMATS §9 : la terminaison `0x00 0x00` du type `0x10` (décision 4) ; renvoi à `ndaal-common::keys`.
- [ ] Chaîne de CLAUDE.md §11, dont `check-linux.sh` ; revue finale par un relecteur neuf ; compte rendu, `.git/pr-body.md`, commandes git et `gh pr create`.
- [ ] Cocher T02.14 après la CI verte.
