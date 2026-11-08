# T02.14 — Encodage des clés internes : spécification

**Tâche :** T02.14 ([CAHIER_DES_TACHES.md](../../../CAHIER_DES_TACHES.md#j02--noyau-de-stockage)).

**Source :** [FORMATS §9](../../../FORMATS_DE_DONNEES.md#9-encodage-des-clés-internes), qui fixe le format ; cette spécification fixe l'interface et les tests.

**Statut :** conception validée par le responsable du projet le 2026-11-08.

## 1. Contexte et périmètre

Toutes les entrées ordonnées d'une partition partagent un espace de clés internes trié par octets. Il est fait d'un octet de type, du `namespace_id` sur 4 octets gros-boutistes, puis d'une suite propre au type. Les clés utilisateur y sont échappées, et les entiers y sont gros-boutistes, pour que **l'ordre des octets soit l'ordre logique**.

**Critère de fin :** la propriété « ordre des octets = ordre logique » est vérifiée.

**Dans le périmètre :** les primitives, et leur composition dans les tests.

**Hors du périmètre :** les constructeurs de clé par type. Chaque moteur composera la sienne : T03 pour le clé-valeur, J04 pour l'objet, J05 pour le vectoriel. Certains champs (`upload_id`, `modèle`, `référence`) ne sont pas encore définis, et CLAUDE.md §8 interdit l'abstraction spéculative.

## 2. Décisions

| # | Décision | Raison | Coût |
|---|---|---|---|
| 1 | Module **`ndaal-common::keys`** | Les moteurs (`ndaal-kv`, `ndaal-object`, `ndaal-vector`) ne dépendent que de `ndaal-common` | — |
| 2 | Même style que `ndaal-common::bytes` : `put_*` ajoute à un `Vec<u8>` ; `get_*` lit un `&mut &[u8]`, n'avance qu'en cas de succès et rend `DecodeError` | Une seule façon de coder et de décoder dans le dépôt | — |
| 3 | **`get_escaped` rend un `Cow<[u8]>`** : emprunt sans copie si la clé ne contient aucun `0x00` | Chemin court sans allocation (CLAUDE.md §4.5) | L'appelant manipule un `Cow` |
| 4 | Pour le type `0x10`, le `0x00 0x00` de FORMATS §9 **est la terminaison** de l'échappement, pas deux octets de plus | La clé échappée se termine déjà par `0x00 0x00` | Une phrase précisée dans FORMATS |

## 3. Interface

```rust
pub enum KeyType {
    Kv = 0x01, Expiry = 0x02, ObjectMeta = 0x10, Multipart = 0x11,
    Staged = 0x12, VectorPayload = 0x20, LinkIndex = 0x30, System = 0x7F,
}

pub fn put_prefix(out: &mut Vec<u8>, kind: KeyType, namespace: NamespaceId);
pub fn get_prefix(input: &mut &[u8]) -> Result<(KeyType, NamespaceId), DecodeError>;
pub fn put_escaped(out: &mut Vec<u8>, key: &[u8]);
pub fn get_escaped<'a>(input: &mut &'a [u8]) -> Result<Cow<'a, [u8]>, DecodeError>;
pub fn put_u32_be(out: &mut Vec<u8>, v: u32);
pub fn put_u64_be(out: &mut Vec<u8>, v: u64);
pub fn get_u32_be(input: &mut &[u8]) -> Result<u32, DecodeError>;
pub fn get_u64_be(input: &mut &[u8]) -> Result<u64, DecodeError>;
pub fn inverted(version: u64) -> u64;   // u64::MAX - version : la plus recente en premier
```

**Échappement :** chaque `0x00` de la clé devient `0x00 0xFF`, et la clé se termine par `0x00 0x00`.

**Refus**, toujours en `DecodeError` et avec l'entrée laissée intacte :
- un type de clé inconnu ;
- un `0x00` suivi d'autre chose que `0x00` ou `0xFF` ;
- une clé sans terminaison ;
- une entrée trop courte.

## 4. Tests

**Propriété (`proptest`, avec un modèle) :**
- `cmp(esc(a), esc(b)) == cmp(a, b)` pour deux clés quelconques ;
- `cmp(esc(a) ++ x, esc(b) ++ y) == cmp((a, x), (b, y))` : l'échappement est sans préfixe commun trompeur, donc un suffixe ne change pas l'ordre ;
- les entiers gros-boutistes suivent l'ordre numérique, et `inverted` l'inverse ;
- une clé composée (type, espace de noms, `db`, clé échappée, échéance) suit l'ordre du n-uplet.

**Unitaires :**
- aller-retour de chaque primitive ;
- `Cow::Borrowed` quand la clé ne contient aucun `0x00` ;
- refus de chaque forme invalide, avec l'entrée intacte.

**Fuzzing :** le corps `common_codec` vérifie aussi que toute clé que `get_escaped` accepte se ré-encode exactement en les octets consommés. Deux entrées sont ajoutées au corpus.

## 5. Documents

FORMATS §9 : la précision sur le type `0x10` (décision 4) et le renvoi à `ndaal-common::keys`.
