# API et protocoles — Ndaal

> Référence des interfaces exposées par Ndaal. Les exigences correspondantes sont dans [CAHIER_DES_CHARGES.md](CAHIER_DES_CHARGES.md) (EF-KV, EF-VEC, EF-OBJ, EF-TX, EF-GEN, EF-OPS).

## Sommaire

1. [Ports et points d'accès](#1-ports-et-points-daccès)
2. [Conventions communes HTTP](#2-conventions-communes-http)
3. [Protocole RESP (clé-valeur)](#3-protocole-resp-clé-valeur)
4. [API S3 (objets)](#4-api-s3-objets)
5. [API vectorielle](#5-api-vectorielle)
6. [API transactionnelle](#6-api-transactionnelle)
7. [Flux de changements](#7-flux-de-changements)
8. [API d'administration](#8-api-dadministration)
9. [Services gRPC](#9-services-grpc)
10. [Codes d'erreur](#10-codes-derreur)
11. [Limites par défaut](#11-limites-par-défaut)

---

## 1. Ports et points d'accès

| Port par défaut | Protocole | Usage | Option |
|---|---|---|---|
| 6379 | RESP2/RESP3 (TCP, TLS optionnel) | Clé-valeur, commandes `NDAAL.*` | `resp.listen` |
| 9000 | HTTP S3 | Objets | `s3.listen` |
| 8080 | HTTP/1.1, HTTP/2 (JSON) | API vectorielle, transactions, objets provisoires, flux de changements, administration, santé, métriques, console web (`/console`) | `http.listen` |
| 8081 | gRPC (HTTP/2) | Mêmes services qu'en HTTP | `grpc.listen` |
| 7000 | Protocole interne (mTLS) | Communication entre nœuds | `cluster.listen` |

Par défaut, tous les ports écoutent sur `127.0.0.1` (ENF-SEC-04).

---

## 2. Conventions communes HTTP

### 2.1 Authentification

| Méthode | En-tête |
|---|---|
| Jeton porteur | `Authorization: Bearer stk_…` |
| Signature AWS SigV4 (service `ndaal`) | `Authorization: AWS4-HMAC-SHA256 …` |
| Session de console (navigateur uniquement) | Cookie `ndaal_session` (`HttpOnly`, `Secure`, `SameSite=Strict`) + en-tête `X-CSRF-Token` pour toute requête qui modifie l'état |

### 2.2 En-têtes Ndaal

| En-tête | Sens | Valeurs |
|---|---|---|
| `x-ndaal-namespace` | Espace de noms (sinon celui de l'URL ou de l'utilisateur) | nom |
| `x-ndaal-durability` | Durabilité exigée pour une écriture | `none`, `async`, `sync`, `replicated` |
| `x-ndaal-consistency` | Cohérence de lecture | `leader` (défaut), `follower`, `lsn:<n>` |
| `x-ndaal-idempotency-key` | Déduplication des réessais d'écriture (24 h) | chaîne ≤ 128 caractères |
| `x-ndaal-lsn` (réponse) | LSN de validation de l'écriture | entier |
| `x-ndaal-partition` (réponse) | Partition ayant traité la requête | entier |
| `x-request-id` | Identifiant de requête (généré si absent, propagé dans les traces) | chaîne |

### 2.3 Format des erreurs

```json
{
  "error": {
    "code": "PRECONDITION_FAILED",
    "message": "La précondition n°0 a échoué : l'objet existe déjà",
    "retryable": false,
    "details": { "precondition_index": 0, "actual_etag": "\"9b2cf535f27731c974343645a3985328\"" },
    "request_id": "01J9Z4…"
  }
}
```

### 2.4 Pagination

Les listes renvoient `{ "items": [...], "next_page_token": "…" }`. Passer `page_token` pour la page suivante ; `limit` (défaut 100, max 1 000).

### 2.5 Versionnement de l'API

Préfixe de chemin `/v1/`. Les ajouts compatibles (nouveaux champs optionnels) ne changent pas le préfixe ; toute rupture introduit `/v2/` avec maintien de `/v1/` pendant au moins deux publications majeures.

---

## 3. Protocole RESP (clé-valeur)

### 3.1 Connexion et espace de noms

```
HELLO 3 AUTH alice s3cr3t          # négociation RESP3 + authentification
NDAAL.NS acme                     # choisir l'espace de noms (sinon : celui de l'utilisateur, ou "default")
SELECT 2                           # base logique 0–15 dans l'espace de noms
```

### 3.2 Commandes supportées

La liste détaillée par type figure dans le cahier des charges (EF-KV-02 à EF-KV-14). La liste exacte, générée depuis la table de commandes, est accessible par `COMMAND DOCS` et publiée dans la documentation utilisateur (ENF-DOC-03).

| Famille | Statut |
|---|---|
| Chaînes, expiration, gestion des clés | Supporté |
| Hash, listes, ensembles, ensembles triés | Supporté |
| `MULTI` / `EXEC` / `WATCH` | Supporté (une partition par transaction) |
| Connexion, `INFO`, `CLIENT`, `CONFIG` (sous-ensemble), `SLOWLOG`, `ACL` (sous-ensemble) | Supporté |
| Pub/Sub, Streams | Supporté (fonctions avancées) |
| `CLUSTER SLOTS` / `CLUSTER SHARDS` / `CLUSTER INFO` / `CLUSTER NODES` | Supporté en mode Redis Cluster |
| `EVAL`, `EVALSHA`, `FUNCTION`, `SCRIPT` | **Non supporté** (`-ERR unsupported command`) |
| `MODULE`, `DEBUG` (sauf `DEBUG SLEEP 0`), `SYNC`, `PSYNC`, `REPLICAOF`, `SAVE`, `BGSAVE`, `BGREWRITEAOF` | **Non supporté** ; `SAVE`/`BGSAVE` renvoient `+OK` et déclenchent un point de contrôle si `resp.compat_save=true` |
| `OBJECT ENCODING/FREQ/IDLETIME`, `MEMORY USAGE` | Supporté (valeurs propres à Ndaal) |
| Bitmaps (`SETBIT`, `GETBIT`, `BITCOUNT`, `BITPOS`, `BITOP`), HyperLogLog (`PFADD`, `PFCOUNT`, `PFMERGE`) | Supporté (fonctions avancées) |
| Géo (`GEOADD`, `GEOSEARCH`, …) | Supporté (fonctions avancées) |

### 3.3 Différences de comportement documentées

| Sujet | Redis | Ndaal |
|---|---|---|
| Persistance | RDB/AOF | Journal unifié ; durabilité par préfixe/espace de noms |
| `KEYS *` | Bloquant | Désactivé par défaut au-delà de 10 000 clés (`resp.keys_max`), renvoie une erreur conseillant `SCAN` |
| `FLUSHALL` | Toutes les bases | Toutes les bases **de l'espace de noms courant** |
| `SELECT` | 16 bases globales | 16 bases par espace de noms |
| `INFO` | Sections Redis | Sections Redis compatibles + section `# Ndaal` |
| Transactions sur plusieurs partitions | `CROSSSLOT` en cluster | `CROSSSLOT` dans les espaces de noms répartis |

### 3.4 Commandes propres à Ndaal

| Commande | Syntaxe | Description |
|---|---|---|
| `NDAAL.NS` | `NDAAL.NS <nom>` | Choisit l'espace de noms de la connexion |
| `NDAAL.DURABILITY` | `NDAAL.DURABILITY none\|async\|sync\|replicated` | Durabilité des écritures suivantes de la connexion |
| `NDAAL.LSN` | `NDAAL.LSN` | LSN de la dernière écriture de la connexion |
| `NDAAL.WAITLSN` | `NDAAL.WAITLSN <lsn> <timeout_ms>` | Attend que la partition ait appliqué ce LSN (lecture de ses écritures sur un réplica) |
| `NDAAL.TX` | `NDAAL.TX <json>` | Transaction multi-modèles (même format que §6.2) ; renvoie le résultat JSON |
| `NDAAL.VSEARCH` | `NDAAL.VSEARCH <collection> <k> VECTOR <f32 binaire\|json> [FILTER <json>] [WITHPAYLOAD]` | Recherche vectorielle simplifiée |
| `NDAAL.VUPSERT` | `NDAAL.VUPSERT <collection> <id> VECTOR <…> [PAYLOAD <json>]` | Insertion d'un point |
| `NDAAL.INFO` | `NDAAL.INFO [partition\|tiers\|memory]` | Informations propres à Ndaal |

---

## 4. API S3 (objets)

### 4.1 Opérations

| Opération | Statut | Notes |
|---|---|---|
| `ListBuckets`, `CreateBucket`, `DeleteBucket`, `HeadBucket`, `GetBucketLocation` | Supporté | Région unique configurable (`s3.region`, défaut `us-east-1`) |
| `PutObject`, `GetObject`, `HeadObject`, `DeleteObject`, `DeleteObjects`, `CopyObject` | Supporté | `CopyObject` sans recopie des blocs (référence) dans un même espace de noms |
| `ListObjectsV2`, `ListObjects` | Supporté | |
| Multipart (7 opérations) | Supporté | `UploadPartCopy` par référence quand possible |
| `GetObjectTagging`, `PutObjectTagging`, `DeleteObjectTagging` | Supporté | |
| `PutBucketVersioning`, `GetBucketVersioning`, `ListObjectVersions` | Supporté (fonction avancée) | |
| `PutBucketLifecycleConfiguration`, `GetBucketLifecycleConfiguration`, `DeleteBucketLifecycle` | Supporté (fonction avancée) | Actions : `Expiration`, `NoncurrentVersionExpiration`, `AbortIncompleteMultipartUpload`, `Transition` (vers classe `COLD` = niveau objet) |
| `PutBucketPolicy`, `GetBucketPolicy`, `DeleteBucketPolicy`, `GetBucketPolicyStatus` | Supporté | Sous-ensemble : `Effect`, `Principal`, `Action`, `Resource`, conditions `aws:SourceIp`, `s3:prefix`, `aws:SecureTransport` |
| `PutBucketCors`, `GetBucketCors`, `DeleteBucketCors` | Supporté | |
| `PutObjectLockConfiguration`, `PutObjectRetention`, `PutObjectLegalHold` (+ `Get*`) | Supporté (fonction avancée) | |
| `PutBucketNotificationConfiguration` | Supporté (fonction avancée) | Cibles : stream Ndaal, webhook |
| `GetObjectAttributes` | Supporté | |
| `PutBucketEncryption` | Supporté | Le chiffrement au repos Ndaal s'applique ; SSE-C supporté (clé fournie par le client) |
| `SelectObjectContent`, `RestoreObject`, Analytics, Inventory, Replication, Website, Logging, Accelerate, RequestPayment, Ownership controls | **Non supporté** | `501 NotImplemented` |

### 4.2 Particularités

- **Noms de compartiments** : règles S3 (3–63 caractères, minuscules, chiffres, points, tirets) ; uniques dans le cluster.
- **Adressage** : chemin (`http://hôte:9000/bucket/clé`) et hôte virtuel (`http://bucket.s3.domaine/clé`) si `s3.domain` est configuré.
- **Espace de noms d'un compartiment** : fourni à la création par l'en-tête `x-ndaal-namespace` (sinon espace par défaut de l'utilisateur).
- **Classes de stockage** : `STANDARD` (placement automatique), `HOT` (épinglé NVMe), `COLD` (niveau objet). Les autres classes AWS sont acceptées et traduites en `STANDARD`.
- **En-têtes de réponse Ndaal** : `x-ndaal-lsn`, `x-ndaal-tier` (niveau ayant servi la lecture).

---

## 5. API vectorielle

Préfixe : `/v1/namespaces/{ns}/collections`.

### 5.1 Collections

**Créer**

```http
PUT /v1/namespaces/acme/collections/chunks
Content-Type: application/json

{
  "vectors": {
    "dense": {
      "size": 768,
      "distance": "cosine",
      "datatype": "f32",
      "index": { "type": "hnsw", "m": 16, "ef_construct": 100 },
      "quantization": { "type": "int8", "always_ram": true, "rescore": true }
    }
  },
  "sparse_vectors": {
    "bm25": { "modifier": "idf" }
  },
  "payload_indexes": {
    "tenant_id": { "type": "keyword", "is_tenant": true },
    "created_at": { "type": "datetime" },
    "lang": { "type": "keyword" },
    "text": { "type": "text", "tokenizer": "word", "lowercase": true }
  },
  "optimizer": {
    "freeze_threshold_points": 200000,
    "full_scan_threshold": 10000,
    "deleted_threshold": 0.2
  },
  "placement": { "tier": "auto" }
}
```

Réponse : `201 Created` avec la description complète.

| Méthode | Chemin | Action |
|---|---|---|
| `GET` | `/collections` | Lister |
| `GET` | `/collections/{c}` | Décrire (configuration, nombre de points, segments, état d'indexation) |
| `PATCH` | `/collections/{c}` | Modifier les paramètres modifiables (optimiseur, `ef` par défaut, quantification → reconstruction en arrière-plan) |
| `DELETE` | `/collections/{c}` | Supprimer |
| `PUT` | `/collections/{c}/payload-indexes/{champ}` | Créer un index de charge utile |
| `DELETE` | `/collections/{c}/payload-indexes/{champ}` | Supprimer un index |

### 5.2 Points

**Insérer ou mettre à jour**

```http
PUT /v1/namespaces/acme/collections/chunks/points?wait=true
Content-Type: application/json

{
  "points": [
    {
      "id": 1001,
      "vectors": { "dense": [0.012, -0.044, …] },
      "payload": { "tenant_id": "t1", "doc": "{doc:42}/contrat.pdf", "page": 3, "lang": "fr" },
      "link": { "bucket": "acme-docs", "key": "{doc:42}/contrat.pdf" }
    }
  ],
  "routing_key": "doc:42"
}
```

`wait=true` : réponse après application (visible aux recherches) ; sinon après durabilité.

| Méthode | Chemin | Action |
|---|---|---|
| `POST` | `/collections/{c}/points/get` | Lire par identifiants (`ids`, `with_payload`, `with_vectors`) |
| `POST` | `/collections/{c}/points/delete` | Supprimer par `ids` ou par `filter` |
| `POST` | `/collections/{c}/points/payload` | Fusionner une charge utile (`ids` ou `filter`) |
| `PUT` | `/collections/{c}/points/payload` | Remplacer la charge utile |
| `POST` | `/collections/{c}/points/payload/delete` | Supprimer des clés de charge utile |
| `PUT` | `/collections/{c}/points/vectors` | Mettre à jour des vecteurs nommés |
| `POST` | `/collections/{c}/points/count` | Compter (`filter`, `exact`) |
| `POST` | `/collections/{c}/points/scroll` | Parcourir (`filter`, `limit`, `offset` = identifiant de reprise, `order_by`) |

### 5.3 Recherche

```http
POST /v1/namespaces/acme/collections/chunks/search
Content-Type: application/json

{
  "vector": { "name": "dense", "values": [0.01, …] },
  "limit": 10,
  "offset": 0,
  "filter": {
    "must": [
      { "key": "tenant_id", "match": { "value": "t1" } },
      { "key": "created_at", "range": { "gte": "2026-01-01T00:00:00Z" } }
    ],
    "must_not": [
      { "key": "lang", "match": { "any": ["de", "it"] } }
    ]
  },
  "params": { "ef": 128, "exact": false, "quantization": { "rescore": true, "oversampling": 2.0 } },
  "score_threshold": 0.3,
  "with_payload": ["doc", "page"],
  "with_vectors": false,
  "consistency": "default",
  "explain": false
}
```

Réponse :

```json
{
  "result": [
    { "id": 1001, "score": 0.873, "payload": { "doc": "{doc:42}/contrat.pdf", "page": 3 } }
  ],
  "lsn": 58211,
  "time_ms": 3.2
}
```

**Grammaire des filtres**

| Condition | Forme |
|---|---|
| Égalité | `{ "key": "k", "match": { "value": v } }` |
| Appartenance | `{ "key": "k", "match": { "any": [a, b] } }` / exclusion `"except"` |
| Texte | `{ "key": "k", "match": { "text": "mots à chercher" } }` |
| Plage | `{ "key": "k", "range": { "gt", "gte", "lt", "lte" } }` (nombres ou dates RFC 3339) |
| Existence / vide / nul | `{ "is_empty": { "key": "k" } }`, `{ "is_null": { "key": "k" } }` |
| Identifiants | `{ "has_id": [1, 2, 3] }` |
| Géo | `{ "key": "loc", "geo_radius": { "center": { "lat", "lon" }, "radius": 1000 } }`, `geo_bounding_box` |
| Imbrication | `{ "must": [...], "should": [...], "must_not": [...], "min_should": { "conditions": [...], "min_count": 2 } }` |
| Champs imbriqués | Chemin pointé `"meta.author"` ; tableaux `"tags[]"` |

**Autres requêtes**

| Méthode | Chemin | Action |
|---|---|---|
| `POST` | `/collections/{c}/search/batch` | Plusieurs recherches en un appel |
| `POST` | `/collections/{c}/recommend` | `positive`, `negative` (identifiants ou vecteurs), stratégie `average_vector` ou `best_score` |
| `POST` | `/collections/{c}/query` | Requête hybride : `prefetch` (plusieurs sous-recherches dense/creux) + `fusion: "rrf"` ou re-classement |

### 5.4 Instantanés de collection

| Méthode | Chemin | Action |
|---|---|---|
| `POST` | `/collections/{c}/snapshots` | Créer (écrit dans un compartiment indiqué ou renvoie un identifiant téléchargeable) |
| `GET` | `/collections/{c}/snapshots` | Lister |
| `POST` | `/collections/{c}/snapshots/restore` | Restaurer depuis un instantané (compartiment/clé ou URL) |

---

## 6. API transactionnelle

### 6.1 Objets provisoires

```http
POST /v1/namespaces/acme/staging
Content-Type: application/json

{ "size_hint": 12582912, "content_type": "application/pdf", "ttl_seconds": 86400, "routing_key": "doc:42" }
```

```json
{
  "staged_id": "stg_01J9Z5K8M3…",
  "upload": {
    "method": "PUT",
    "url": "http://127.0.0.1:8080/v1/namespaces/acme/staging/stg_01J9Z5K8M3…/data",
    "multipart": { "create_url": "…/multipart", "part_size_min": 5242880 }
  },
  "expires_at": "2026-10-13T10:00:00Z"
}
```

Le téléversement peut aussi utiliser le protocole S3 sur le compartiment virtuel `.staging-<ns>` avec la clé `stg_…`.

### 6.2 Exécuter une transaction

```http
POST /v1/namespaces/acme/tx
Content-Type: application/json
x-ndaal-idempotency-key: ingest-doc-42-essai-1

{
  "routing_key": "doc:42",
  "durability": "replicated",
  "preconditions": [
    { "type": "object_absent", "bucket": "acme-docs", "key": "{doc:42}/contrat.pdf" },
    { "type": "kv_equals", "key": "doc:{doc:42}:etat", "value": "en_cours" }
  ],
  "operations": [
    { "op": "object.commit_staged", "staged_id": "stg_01J9Z5K8M3…", "bucket": "acme-docs", "key": "{doc:42}/contrat.pdf",
      "metadata": { "x-amz-meta-source": "upload-web" } },
    { "op": "vector.upsert", "collection": "chunks", "points": [ { "id": 1001, "vectors": { "dense": [ … ] }, "payload": { … } } ],
      "link": { "bucket": "acme-docs", "key": "{doc:42}/contrat.pdf" } },
    { "op": "kv.del_prefix", "prefix": "cache:{doc:42}:" },
    { "op": "kv.set", "key": "doc:{doc:42}:etat", "value": "indexe", "ttl_seconds": null }
  ],
  "return": ["results", "lsn"]
}
```

Réponse :

```json
{
  "committed": true,
  "lsn": 58212,
  "tx_id": "0000000700000000000e3664",
  "results": [
    { "op": 0, "version_id": "06J9Z5Q…", "etag": "\"5d41402abc4b2a76b9719d911017c592\"" },
    { "op": 1, "upserted": 1 },
    { "op": 2, "deleted": 7 },
    { "op": 3, "ok": true }
  ],
  "derived_operations": [
    { "rule": "invalidation#0", "op": "kv.del_prefix", "prefix": "cache:recherche:", "deleted": 12 }
  ]
}
```

### 6.3 Catalogue des opérations

| Opération | Paramètres |
|---|---|
| `kv.set` | `key`, `value` (chaîne ou base64 via `value_b64`), `ttl_seconds`, `db`, `link` |
| `kv.del` | `keys[]`, `db` |
| `kv.del_prefix` | `prefix`, `db` (préfixe déclaré ou limité par `max_keys`) |
| `kv.incr` | `key`, `by` (entier ou flottant) |
| `kv.expire` | `key`, `ttl_seconds` |
| `kv.hset` / `kv.hdel` | `key`, `fields` |
| `kv.lpush` / `kv.rpush` | `key`, `items[]` |
| `kv.sadd` / `kv.srem` | `key`, `members[]` |
| `kv.zadd` / `kv.zrem` | `key`, `entries[]` / `members[]` |
| `vector.upsert` | `collection`, `points[]`, `link` |
| `vector.delete` | `collection`, `ids[]` ou `filter` (borné par `max_points`) |
| `vector.set_payload` | `collection`, `ids[]`, `payload` |
| `object.commit_staged` | `staged_id`, `bucket`, `key`, `metadata`, `tags`, `content_type` |
| `object.put_inline` | `bucket`, `key`, `data_b64` (≤ 64 Kio), `content_type` |
| `object.delete` | `bucket`, `key`, `version_id`, `cascade` |
| `object.copy` | `from_bucket`, `from_key`, `bucket`, `key` |
| `object.set_tags` | `bucket`, `key`, `tags` |

### 6.4 Préconditions

| Type | Paramètres | Vrai si |
|---|---|---|
| `kv_exists` / `kv_absent` | `key` | La clé existe / n'existe pas |
| `kv_equals` | `key`, `value` | La valeur est égale |
| `kv_version` | `key`, `version` | La version (LSN) de la clé est égale |
| `object_exists` / `object_absent` | `bucket`, `key` | |
| `object_etag` | `bucket`, `key`, `etag` | L'ETag courant est égal |
| `point_exists` / `point_absent` | `collection`, `id` | |
| `point_version` | `collection`, `id`, `version` | |
| `lsn_at_most` | `lsn` | Aucune modification de la partition depuis ce LSN (verrou optimiste global) |

---

## 7. Flux de changements

```http
GET /v1/namespaces/acme/changes?from_lsn=58000&models=object,vector&format=ndjson
Accept: application/x-ndjson
```

Chaque ligne :

```json
{ "lsn": 58212, "partition": 7, "ts": "2026-10-12T10:00:00.123Z", "model": "object", "op": "put", "bucket": "acme-docs", "key": "{doc:42}/contrat.pdf", "version_id": "06J9Z5Q…", "tx_id": "…" }
```

- Disponible en Server-Sent Events (`Accept: text/event-stream`), NDJSON et gRPC (flux).
- Reprise par `from_lsn` (par partition dans un espace réparti : `from_lsn=7:58000,8:41000`).
- Limité par la rétention du journal ; au-delà : `410 Gone` avec le plus ancien LSN disponible.

---

## 8. API d'administration

Préfixe `/v1/admin` (permission `ADMIN`).

| Méthode | Chemin | Action |
|---|---|---|
| `GET` | `/health/live`, `/health/ready` | Sondes (sans authentification) |
| `GET` | `/metrics` | Métriques Prometheus (authentification configurable) |
| `GET` | `/admin/cluster` | État du cluster : nœuds, rôles, zones, version, drapeaux de fonctionnalités |
| `POST` | `/admin/cluster/init` | Initialiser un cluster (premier nœud) |
| `POST` | `/admin/nodes/{id}/drain` | Vider un nœud (déplacer ses réplicas et leaders) |
| `DELETE` | `/admin/nodes/{id}` | Retirer un nœud vidé |
| `GET` | `/admin/partitions` | Partitions, réplicas, leaders, LSN, retards |
| `POST` | `/admin/partitions/{id}/transfer-leader` | Transférer le leadership |
| `POST` | `/admin/partitions/{id}/split` | Diviser |
| `POST` | `/admin/partitions/{id}/compact` | Forcer un compactage |
| `GET`/`PUT`/`DELETE` | `/admin/namespaces/{ns}` | Gérer un espace de noms (mode, quotas, durabilité, règles) |
| `GET`/`PUT`/`DELETE` | `/admin/users/{nom}` | Gérer un utilisateur |
| `POST` | `/admin/users/{nom}/credentials` | Créer une clé d'accès / un jeton (secret affiché une seule fois) |
| `GET`/`PUT`/`DELETE` | `/admin/roles/{nom}` | Gérer un rôle |
| `GET`/`PATCH` | `/admin/config` | Configuration effective ; modification des paramètres modifiables à chaud |
| `POST` | `/admin/backups` | Lancer une sauvegarde (`full` ou `incremental`, destination) |
| `GET` | `/admin/backups` | Lister les sauvegardes |
| `POST` | `/admin/restore` | Restaurer (sauvegarde, LSN ou horodatage cible, espaces de noms) |
| `GET` | `/admin/slowlog` | Requêtes lentes |
| `GET` | `/admin/audit` | Journal d'audit (flux) |
| `POST` | `/admin/diagnostics` | Générer une archive de diagnostic |
| `POST` | `/admin/keys/rotate` | Rotation de la clé maîtresse ou des clés de données |

### 8.1 Points d'accès pour la console et les outils interactifs

Ces points d'accès sont **publics et documentés** comme le reste de l'API : la console n'utilise rien d'autre (ENF-UI-05). Ils appliquent les mêmes contrôles d'autorisation que RESP, S3 et l'API vectorielle.

| Méthode | Chemin | Action |
|---|---|---|
| `POST` / `GET` / `DELETE` | `/v1/auth/session` | Ouvrir une session (utilisateur et mot de passe), lire l'utilisateur courant et ses droits, fermer la session |
| `GET` | `/v1/namespaces/{ns}/kv/keys` | Parcourir les clés (`match`, `type`, `cursor`, `count`), équivalent de `SCAN` |
| `GET` / `PUT` / `DELETE` | `/v1/namespaces/{ns}/kv/keys/{clé}` | Lire (type, valeur paginée pour les structures, TTL, taille), écrire, supprimer une clé |
| `POST` | `/v1/namespaces/{ns}/kv/command` | Exécuter une commande RESP (`{"args": ["HGETALL", "user:42"]}`) avec la même table de commandes et les mêmes ACL que le port RESP ; les commandes bloquantes et `SUBSCRIBE` sont refusées |
| `GET` | `/v1/namespaces/{ns}/buckets/{b}/objects` | Lister les objets (`prefix`, `delimiter`, `cursor`), versions comprises |
| `POST` | `/v1/namespaces/{ns}/buckets/{b}/presign` | Obtenir des URL présignées S3 (envoi simple ou multipart, téléchargement) ; le navigateur transfère ensuite les octets directement sur le port S3 |
| `GET` | `/v1/namespaces/{ns}/links` | Liens entre modèles à partir d'un objet, d'un point ou d'une clé (`object`, `point`, `key`), dans les deux sens (EF-OPS-14) |
| `POST` | `/v1/namespaces/{ns}/tx?dry_run=true` | Simuler une transaction : vérifie autorisations et préconditions, renvoie les effets prévus et les règles déclenchées, sans rien écrire (EF-OPS-14) |

Les URL présignées produites pour la console ont une durée de vie courte (15 min par défaut). Le port S3 accepte automatiquement les requêtes CORS venant de l'origine de la console du même nœud.

---

## 9. Services gRPC

```protobuf
syntax = "proto3";
package ndaal.v1;

service Vectors {
  rpc CreateCollection(CreateCollectionRequest) returns (Collection);
  rpc Upsert(UpsertRequest) returns (WriteResult);
  rpc Delete(DeletePointsRequest) returns (WriteResult);
  rpc Get(GetPointsRequest) returns (GetPointsResponse);
  rpc Search(SearchRequest) returns (SearchResponse);
  rpc SearchBatch(SearchBatchRequest) returns (SearchBatchResponse);
  rpc Query(QueryRequest) returns (SearchResponse);
  rpc Scroll(ScrollRequest) returns (ScrollResponse);
  rpc UpsertStream(stream UpsertRequest) returns (WriteResult);   // ingestion massive
}

service Transactions {
  rpc Stage(StageRequest) returns (StageResponse);
  rpc UploadStaged(stream StagedChunk) returns (StagedUploadResult);
  rpc Commit(TxRequest) returns (TxResponse);
}

service Changes {
  rpc Subscribe(SubscribeRequest) returns (stream ChangeEvent);
}

service Admin {
  rpc ClusterStatus(Empty) returns (ClusterStatusResponse);
  // … mêmes opérations que l'API HTTP d'administration
}
```

Les vecteurs sont transmis en `bytes` (f32 petit-boutiste) pour éviter le coût de `repeated float`.

---

## 10. Codes d'erreur

| Code Ndaal | HTTP | gRPC | RESP | S3 | Réessayable |
|---|---|---|---|---|---|
| `INVALID_ARGUMENT` | 400 | `INVALID_ARGUMENT` | `-ERR …` | `InvalidArgument` / `InvalidRequest` | Non |
| `UNAUTHENTICATED` | 401 | `UNAUTHENTICATED` | `-NOAUTH` / `-WRONGPASS` | `InvalidAccessKeyId` / `SignatureDoesNotMatch` | Non |
| `PERMISSION_DENIED` | 403 | `PERMISSION_DENIED` | `-NOPERM` | `AccessDenied` | Non |
| `NOT_FOUND` | 404 | `NOT_FOUND` | (nil) | `NoSuchKey` / `NoSuchBucket` / `NoSuchUpload` | Non |
| `ALREADY_EXISTS` | 409 | `ALREADY_EXISTS` | — | `BucketAlreadyOwnedByYou` / `BucketAlreadyExists` | Non |
| `PRECONDITION_FAILED` | 412 | `FAILED_PRECONDITION` | `(nil)` pour `EXEC` après `WATCH` | `PreconditionFailed` | Non |
| `CROSS_PARTITION` | 400 | `INVALID_ARGUMENT` | `-CROSSSLOT` | — | Non |
| `TX_TOO_LARGE` | 413 | `INVALID_ARGUMENT` | `-ERR transaction too large` | `EntityTooLarge` | Non |
| `QUOTA_EXCEEDED` | 429 | `RESOURCE_EXHAUSTED` | `-ERR quota exceeded` | `QuotaExceeded` (403) | Après délai |
| `OVERLOADED` | 429 / 503 | `RESOURCE_EXHAUSTED` / `UNAVAILABLE` | `-BUSY` | `SlowDown` (503) | Oui |
| `NOT_LEADER` | 503 (interne, normalement masqué) | `UNAVAILABLE` | `-MOVED` (mode cluster) / `-TRYAGAIN` | `ServiceUnavailable` | Oui |
| `OUT_OF_MEMORY` | 507 | `RESOURCE_EXHAUSTED` | `-OOM` | `ServiceUnavailable` | Après délai |
| `TIMEOUT` | 504 | `DEADLINE_EXCEEDED` | `-TIMEOUT` | `RequestTimeout` | Oui (avec clé d'idempotence) |
| `UNSUPPORTED` | 501 | `UNIMPLEMENTED` | `-ERR unsupported command` | `NotImplemented` | Non |
| `DATA_CORRUPTION` | 500 | `DATA_LOSS` | `-ERR internal` | `InternalError` | Oui (autre réplica) |
| `INTERNAL` | 500 | `INTERNAL` | `-ERR internal` | `InternalError` | Selon le cas |

---

## 11. Limites par défaut

| Limite | Défaut | Paramètre |
|---|---|---|
| Taille d'une valeur clé-valeur | 512 Mio | `kv.max_value_size` |
| Taille d'une clé | 64 Kio | `kv.max_key_size` |
| Taille d'une commande RESP | 512 Mio | `resp.max_command_size` |
| Connexions par nœud | 100 000 | `net.max_connections` |
| Dimension vectorielle | 65 536 | — |
| Points par requête `upsert` | 10 000 | `vector.max_batch` |
| Taille de charge utile d'un point | 1 Mio | `vector.max_payload_size` |
| Résultats par recherche (`limit + offset`) | 10 000 | `vector.max_results` |
| Taille d'un objet | 5 Tio | — |
| Taille d'un objet en ligne | 64 Kio | `object.inline_threshold` |
| Opérations par transaction | 10 000 | `tx.max_operations` |
| Taille d'une transaction (hors objets provisoires) | 64 Mio | `tx.max_bytes` |
| Durée de vie d'un objet provisoire | 24 h | `tx.staging_ttl` |
| Profondeur JSON | 64 | `http.max_json_depth` |
| Taille d'un corps JSON | 64 Mio | `http.max_body_size` |
