# Cahier des charges — Ndaal

> Document de référence décrivant **ce que** Ndaal doit faire et **avec quel niveau de qualité**.
> La conception (*comment*) est décrite dans [ARCHITECTURE.md](ARCHITECTURE.md). Le découpage en travail est dans [CAHIER_DES_TACHES.md](CAHIER_DES_TACHES.md).

## Sommaire

1. [Contexte et problème](#1-contexte-et-problème)
2. [Objectifs](#2-objectifs)
3. [Utilisateurs et cas d'usage](#3-utilisateurs-et-cas-dusage)
4. [Périmètre](#4-périmètre)
5. [Exigences fonctionnelles](#5-exigences-fonctionnelles)
6. [Exigences non fonctionnelles](#6-exigences-non-fonctionnelles)
7. [Environnement de référence](#7-environnement-de-référence)
8. [Contraintes](#8-contraintes)
9. [Hypothèses](#9-hypothèses)
10. [Risques](#10-risques)
11. [Critères d'acceptation globaux](#11-critères-dacceptation-globaux)
12. [Matrice de traçabilité exigences → jalons](#12-matrice-de-traçabilité-exigences--jalons)

---

## 1. Contexte et problème

Les applications modernes, en particulier celles qui intègrent de l'IA (RAG, recherche sémantique, agents, assistants documentaires), reposent presque toujours sur la même combinaison de services :

- un **stockage objet** compatible S3 pour les fichiers bruts (PDF, images, audio, vidéos, exports) ;
- une **base vectorielle** pour les embeddings et la recherche par similarité ;
- un **stockage clé-valeur en mémoire** pour le cache, les sessions, les compteurs, la limitation de débit et les files légères.

Cette combinaison pose cinq problèmes récurrents :

| # | Problème | Conséquence concrète |
|---|---|---|
| P1 | **Incohérence entre services** | Un fichier existe sans son embedding, un vecteur pointe vers un fichier supprimé, un cache sert un résultat périmé. L'application doit gérer ces cas elle-même, souvent mal. |
| P2 | **Coût d'exploitation multiplié** | Trois déploiements, trois configurations, trois stratégies de sauvegarde, trois tableaux de bord, trois montées de version, trois surfaces d'attaque. |
| P3 | **Ressources mal utilisées** | Chaque service réserve sa propre RAM et ses propres disques. Les pics de l'un ne profitent pas des creux de l'autre. |
| P4 | **Incertitude sur les licences** | Les changements de licence (Redis en 2024, retrait de l'édition communautaire de MinIO) obligent à des migrations non prévues. |
| P5 | **Pas de requête transversale** | Impossible d'exprimer en une seule opération « les documents les plus proches de cette question, depuis le cache s'il est chaud, avec un lien de téléchargement ». |

Les tentatives de regroupement se heurtent à des contraintes opposées : latence de l'ordre de la microseconde pour le cache, calcul intensif pour les vecteurs, débit massif pour les fichiers. Ndaal résout cette opposition par l'architecture (isolation par cœur CPU, stockage par niveaux, journal unique), décrite dans [ARCHITECTURE.md](ARCHITECTURE.md).

---

## 2. Objectifs

| ID | Objectif | Indicateur de réussite |
|---|---|---|
| O1 | Remplacer Redis, une base vectorielle et un stockage S3 par **un seul système** pour les cas d'usage courants | Une application de référence (RAG) fonctionne avec Ndaal seul, sans modification de ses clients Redis et S3 |
| O2 | Offrir des **transactions atomiques entre modèles** (clé, vecteur, objet) | Aucun état incohérent observable après injection de pannes (voir [STRATEGIE_DE_TESTS.md](STRATEGIE_DE_TESTS.md)) |
| O3 | **Isoler** les charges : un transfert massif ne dégrade pas la latence du cache | Dégradation de la latence p99 du cache inférieure à 20 % sous charge de transfert saturante (ENF-ISO-01) |
| O4 | Fonctionner **d'un nœud unique jusqu'à un cluster** sans changer d'outil | Même binaire, même API ; passage d'un nœud à trois sans interruption de service |
| O5 | Être **simple à exploiter** | Démarrage en une commande ; sauvegarde et restauration d'un nœud en une commande |
| O6 | Être **libre et pérenne** | Licence Apache 2.0, dépendances sous licences permissives uniquement |
| O7 | Être **au moins aussi performant** que chaque service remplacé (Redis/Valkey, Qdrant, MinIO), et viser à les dépasser | Bancs comparatifs ENF-COMP-01 à 08 atteints sur la même machine, à durabilité équivalente, résultats publiés |

---

## 3. Utilisateurs et cas d'usage

### 3.1 Profils d'utilisateurs

| Profil | Besoin principal |
|---|---|
| **Développeur d'application** | Utiliser ses clients existants (redis-py, ioredis, boto3, aws-sdk) et un SDK simple pour les vecteurs et les transactions |
| **Équipe IA / données** | Ingérer des documents, calculer des embeddings ailleurs, rechercher avec filtres, garder la cohérence entre fichiers et vecteurs |
| **Opérateur / SRE** | Déployer, superviser, sauvegarder, mettre à jour, dimensionner un seul système |
| **Auto-hébergeur / PME** | Faire tourner l'ensemble sur une seule machine modeste |
| **Contributeur** | Comprendre le code, ajouter une commande ou un format, sans casser la durabilité |

### 3.2 Cas d'usage de référence

| ID | Cas d'usage | Modèles impliqués |
|---|---|---|
| UC-01 | **Ingestion documentaire** : envoyer un PDF, enregistrer ses morceaux (chunks) vectorisés et leurs métadonnées, invalider le cache de recherche, le tout atomiquement | Objet + vecteur + clé |
| UC-02 | **Recherche sémantique avec cache** : rechercher les K documents les plus proches d'une question avec filtres (locataire, date, type), mettre le résultat en cache avec TTL | Vecteur + clé |
| UC-03 | **Suppression conforme (RGPD)** : supprimer un document, tous ses vecteurs et toutes les entrées de cache associées en une opération | Objet + vecteur + clé |
| UC-04 | **Sessions et limitation de débit** : sessions utilisateurs avec expiration, compteurs atomiques par fenêtre de temps | Clé |
| UC-05 | **Médias** : téléversement multipart de gros fichiers (plusieurs Go), lecture par plages (streaming vidéo), URL présignées | Objet |
| UC-06 | **Mémoire d'agent IA** : stocker des souvenirs (texte + embedding + métadonnées), rechercher par similarité, expirer les souvenirs anciens | Vecteur + clé |
| UC-07 | **Recommandation** : vecteurs de produits, recherche des voisins, mise en cache des recommandations par utilisateur | Vecteur + clé |
| UC-08 | **Sauvegarde applicative** : utiliser Ndaal comme cible S3 pour des sauvegardes d'autres systèmes | Objet |
| UC-09 | **Multi-locataire SaaS** : isoler les données, les quotas et les clés d'accès de chaque client | Tous |
| UC-10 | **Déploiement embarqué / edge** : un seul binaire sur une petite machine, sans dépendance externe | Tous |

---

## 4. Périmètre

### 4.1 Inclus

- Stockage clé-valeur en mémoire avec persistance, compatible avec un **sous-ensemble défini** du protocole Redis (RESP2 et RESP3).
- Base vectorielle : collections, insertion, mise à jour, suppression, recherche approximative (ANN) avec filtres, quantification.
- Stockage objet compatible avec un **sous-ensemble défini** de l'API S3, dont le multipart et les URL présignées.
- Transactions atomiques entre les trois modèles au sein d'un espace de noms.
- Stockage par niveaux RAM → NVMe → stockage objet (interne ou externe).
- Réplication, haute disponibilité, cluster multi-nœuds avec rôles.
- Sécurité : TLS, authentification, autorisations, multi-locataire, chiffrement au repos.
- Outils : interface en ligne de commande, API d'administration, métriques, traces, sauvegarde et restauration.
- SDK officiel Rust, puis Python et TypeScript.

### 4.2 Exclus (hors périmètre)

| Exclusion | Raison |
|---|---|
| Langage de script côté serveur (Lua, `EVAL`, Functions Redis) | Complexité et risque de sécurité ; les transactions Ndaal couvrent le besoin principal |
| Compatibilité totale avec toutes les commandes Redis | Un sous-ensemble documenté couvre la grande majorité des usages |
| Compatibilité totale avec toutes les API S3 (Select, Object Lambda, Analytics…) | Idem |
| Calcul des embeddings dans le moteur | Responsabilité de l'application ou d'un service d'inférence ; peut être ajouté plus tard comme extension |
| Moteur SQL ou relationnel | Ndaal ne remplace pas PostgreSQL |
| Transactions entre espaces de noms différents | Coût de coordination trop élevé pour la valeur apportée (voir [ADR-006](DECISIONS.md)) |
| Réplication géographique multi-régions active-active | Pourra faire l'objet d'une étude ultérieure |
| Outils graphiques d'analyse de données (BI, requêtes analytiques, rapports) | La console couvre l'exploitation et l'exploration des données, pas leur analyse |

---

## 5. Exigences fonctionnelles

Chaque exigence porte un identifiant stable, une priorité (**Essentielle**, **Importante**, **Souhaitable**) et un critère de vérification.

### 5.1 Exigences générales (EF-GEN)

| ID | Exigence | Priorité | Vérification |
|---|---|---|---|
| EF-GEN-01 | Ndaal DOIT être livré sous forme d'**un seul binaire** exécutable contenant tous les services. | Essentielle | Le binaire démarre et sert RESP, S3 et l'API vectorielle sans autre processus |
| EF-GEN-02 | Ndaal DOIT organiser les données en **espaces de noms** (*namespaces*). Un espace de noms contient des clés, des collections vectorielles et des compartiments (*buckets*). | Essentielle | Deux espaces de noms ont des données strictement séparées |
| EF-GEN-03 | Un espace de noms par défaut nommé `default` DOIT exister pour les clients qui n'en précisent pas. | Essentielle | Un client Redis sans configuration écrit dans `default` |
| EF-GEN-04 | Ndaal DOIT pouvoir démarrer **sans fichier de configuration**, avec des valeurs par défaut raisonnables. | Importante | `ndaal server --data-dir ./data` suffit |
| EF-GEN-05 | La configuration DOIT pouvoir être fournie par fichier TOML, variables d'environnement et options de ligne de commande, dans cet ordre de priorité croissante. | Importante | Test de priorité des sources |
| EF-GEN-06 | Ndaal DOIT exposer une **API d'administration** HTTP (état, configuration effective, statistiques, opérations de maintenance). | Essentielle | Appels documentés dans [API_ET_PROTOCOLES.md](API_ET_PROTOCOLES.md) |
| EF-GEN-07 | Ndaal DOIT fournir une **interface en ligne de commande** `ndaal` (serveur, client, administration, sauvegarde). | Essentielle | Sous-commandes documentées |
| EF-GEN-08 | Ndaal DOIT s'arrêter proprement sur `SIGTERM` : fin des requêtes en cours dans un délai configurable, vidage du journal, fermeture des fichiers. | Essentielle | Aucune perte de données acquittées après arrêt propre |
| EF-GEN-09 | Ndaal DOIT pouvoir être **intégré comme bibliothèque** Rust (mode embarqué, sans réseau). | Souhaitable | Exemple compilé utilisant la crate `ndaal-embedded` |

### 5.2 Clé-valeur (EF-KV)

| ID | Exigence | Priorité | Vérification |
|---|---|---|---|
| EF-KV-01 | Ndaal DOIT implémenter le protocole **RESP2 et RESP3** (négociation via `HELLO`). | Essentielle | `redis-cli` et les clients officiels se connectent |
| EF-KV-02 | Types **chaînes** : `GET`, `SET` (options `EX`, `PX`, `EXAT`, `PXAT`, `NX`, `XX`, `KEEPTTL`, `GET`), `MGET`, `MSET`, `MSETNX`, `GETSET`, `GETDEL`, `GETEX`, `APPEND`, `STRLEN`, `GETRANGE`, `SETRANGE`, `INCR`, `INCRBY`, `INCRBYFLOAT`, `DECR`, `DECRBY`, `SETNX`, `SETEX`, `PSETEX`. | Essentielle | Suite de compatibilité |
| EF-KV-03 | **Expiration** : `EXPIRE`, `PEXPIRE`, `EXPIREAT`, `PEXPIREAT`, `TTL`, `PTTL`, `PERSIST`, `EXPIRETIME`, `PEXPIRETIME`. Les clés expirées NE DOIVENT JAMAIS être retournées, même avant leur suppression physique. | Essentielle | Test de lecture juste après expiration |
| EF-KV-04 | Gestion des clés : `DEL`, `UNLINK`, `EXISTS`, `TYPE`, `RENAME`, `RENAMENX`, `KEYS` (désactivable), `SCAN` (avec `MATCH`, `COUNT`, `TYPE`), `RANDOMKEY`, `DBSIZE`, `FLUSHDB`, `TOUCH`, `COPY`. | Essentielle | Suite de compatibilité |
| EF-KV-05 | Type **hash** : `HSET`, `HGET`, `HMGET`, `HGETALL`, `HDEL`, `HEXISTS`, `HKEYS`, `HVALS`, `HLEN`, `HINCRBY`, `HINCRBYFLOAT`, `HSETNX`, `HSCAN`, `HSTRLEN`, `HRANDFIELD`. | Essentielle | Suite de compatibilité |
| EF-KV-06 | Type **liste** : `LPUSH`, `RPUSH`, `LPUSHX`, `RPUSHX`, `LPOP`, `RPOP`, `LLEN`, `LRANGE`, `LINDEX`, `LSET`, `LREM`, `LTRIM`, `LINSERT`, `LPOS`, `LMOVE`, `BLPOP`, `BRPOP`, `BLMOVE`. | Importante | Suite de compatibilité ; opérations bloquantes testées |
| EF-KV-07 | Type **ensemble** : `SADD`, `SREM`, `SMEMBERS`, `SISMEMBER`, `SMISMEMBER`, `SCARD`, `SPOP`, `SRANDMEMBER`, `SINTER`, `SUNION`, `SDIFF`, `SINTERSTORE`, `SUNIONSTORE`, `SDIFFSTORE`, `SSCAN`, `SMOVE`. | Importante | Suite de compatibilité |
| EF-KV-08 | Type **ensemble trié** : `ZADD` (options `NX`, `XX`, `GT`, `LT`, `CH`, `INCR`), `ZREM`, `ZSCORE`, `ZMSCORE`, `ZINCRBY`, `ZCARD`, `ZCOUNT`, `ZRANGE` (avec `BYSCORE`, `BYLEX`, `REV`, `LIMIT`), `ZRANK`, `ZREVRANK`, `ZPOPMIN`, `ZPOPMAX`, `ZRANGESTORE`, `ZSCAN`, `ZREMRANGEBYSCORE`, `ZREMRANGEBYRANK`. | Importante | Suite de compatibilité |
| EF-KV-09 | Transactions Redis : `MULTI`, `EXEC`, `DISCARD`, `WATCH`, `UNWATCH`, avec sémantique identique à Redis. | Essentielle | Tests de concurrence avec `WATCH` |
| EF-KV-10 | Commandes de connexion et serveur : `PING`, `ECHO`, `HELLO`, `AUTH`, `SELECT` (0–15, mappés vers des sous-espaces), `CLIENT SETNAME/GETNAME/ID/LIST/KILL/INFO`, `INFO`, `COMMAND` (et `COMMAND DOCS`, `COMMAND COUNT`), `CONFIG GET/SET` (sous-ensemble), `TIME`, `QUIT`, `RESET`. | Essentielle | Les clients usuels s'initialisent sans erreur |
| EF-KV-11 | **Politiques d'éviction** configurables quand la mémoire allouée au cache est pleine : `noeviction`, `allkeys-lru`, `allkeys-lfu`, `volatile-lru`, `volatile-lfu`, `volatile-ttl`, `allkeys-random`, et `tiered` (débordement vers NVMe au lieu de supprimer). | Essentielle | Test de remplissage mémoire |
| EF-KV-12 | Chaque clé DOIT pouvoir être déclarée **volatile** (cache pur, peut être perdue au redémarrage) ou **durable** (journalisée), par configuration d'espace de noms ou par préfixe de clé. | Importante | Test de redémarrage |
| EF-KV-13 | **Pub/Sub** : `PUBLISH`, `SUBSCRIBE`, `UNSUBSCRIBE`, `PSUBSCRIBE`, `PUNSUBSCRIBE`, `PUBSUB CHANNELS/NUMSUB/NUMPAT`. | Souhaitable | Suite de compatibilité |
| EF-KV-14 | **Streams** : `XADD`, `XREAD`, `XRANGE`, `XREVRANGE`, `XLEN`, `XTRIM`, `XDEL`, groupes de consommateurs (`XGROUP`, `XREADGROUP`, `XACK`, `XPENDING`, `XCLAIM`, `XAUTOCLAIM`). | Souhaitable | Suite de compatibilité |
| EF-KV-17 | **Bitmaps** (`SETBIT`, `GETBIT`, `BITCOUNT`, `BITPOS`, `BITOP`), **HyperLogLog** (`PFADD`, `PFCOUNT`, `PFMERGE`) et **géo** (`GEOADD`, `GEOPOS`, `GEODIST`, `GEOSEARCH`, `GEOSEARCHSTORE`). | Souhaitable | Suite de compatibilité |
| EF-KV-15 | Une commande non supportée DOIT renvoyer une erreur explicite `ERR unknown command` ou `ERR unsupported command`, jamais un comportement silencieusement différent. | Essentielle | Test de toutes les commandes Redis listées par `COMMAND` côté Redis |
| EF-KV-16 | Les valeurs DOIVENT pouvoir atteindre **512 Mio** (limite Redis). Les valeurs de plus de 1 Mio DEVRAIENT être stockées hors ligne (segment séparé). | Importante | Test de valeur volumineuse |

### 5.3 Vecteurs (EF-VEC)

| ID | Exigence | Priorité | Vérification |
|---|---|---|---|
| EF-VEC-01 | **Collections** : créer, lister, décrire, modifier (paramètres modifiables), supprimer une collection dans un espace de noms. | Essentielle | API documentée |
| EF-VEC-02 | Une collection DOIT déclarer un ou plusieurs **vecteurs nommés**, chacun avec dimension (1 à 65 536), métrique (`cosine`, `dot`, `euclid`, `manhattan`) et type d'élément (`f32`, `f16`, `u8`). | Essentielle | Validation à la création |
| EF-VEC-03 | **Points** : un point possède un identifiant (entier 64 bits non signé ou UUID), un ou plusieurs vecteurs et une **charge utile** (*payload*) JSON. | Essentielle | Insertion et lecture |
| EF-VEC-04 | Opérations sur les points : `upsert` (par lot), `get` (par identifiants), `delete` (par identifiants ou par filtre), `set_payload`, `delete_payload`, `update_vectors`, `count`, `scroll` (parcours paginé). | Essentielle | API documentée |
| EF-VEC-05 | **Recherche** des K plus proches voisins approximatifs, avec paramètres de précision (`ef`), seuil de score, pagination (`offset`), sélection des champs retournés. | Essentielle | Rappel mesuré (ENF-PERF-04) |
| EF-VEC-06 | **Filtres** sur la charge utile, combinables (`must`, `should`, `must_not`) : égalité, appartenance, plages numériques et de dates, existence, valeur nulle, texte (correspondance de mots), géographie (rayon, rectangle). | Essentielle | Tests de filtres ; résultats identiques à une recherche exhaustive filtrée |
| EF-VEC-07 | **Index de charge utile** déclarables par champ (mot-clé, entier, flottant, booléen, date, géo, texte) pour accélérer les filtres. | Essentielle | Gain mesuré sur filtres sélectifs |
| EF-VEC-08 | La recherche filtrée DOIT conserver un rappel élevé même pour des filtres très sélectifs (moins de 1 % des points) : bascule automatique vers une recherche exhaustive sur le sous-ensemble filtré quand c'est plus efficace. | Essentielle | Rappel ≥ 0,95 avec filtre à 0,5 % de sélectivité |
| EF-VEC-09 | **Quantification** configurable : scalaire (int8), binaire (1 bit), par produit (PQ), avec **re-classement** (*rescoring*) sur les vecteurs complets. | Importante | Rappel et empreinte mémoire mesurés |
| EF-VEC-10 | **Recherche par lots** : plusieurs requêtes dans un appel. | Importante | API documentée |
| EF-VEC-11 | **Recommandation** : recherche à partir d'exemples positifs et négatifs (identifiants de points existants). | Souhaitable | API documentée |
| EF-VEC-12 | **Vecteurs creux** (*sparse*) et **recherche hybride** dense + creux avec fusion des classements (RRF). | Souhaitable | API documentée |
| EF-VEC-13 | **Index sur disque** pour les collections plus grandes que la RAM disponible (graphe de type Vamana/DiskANN avec vecteurs compressés en RAM). | Importante | Collection de 100 M vecteurs sur machine de référence |
| EF-VEC-14 | **Multi-vecteurs** par point (par exemple pour les modèles de type ColBERT) avec score MaxSim. | Souhaitable | API documentée |
| EF-VEC-15 | Les écritures DOIVENT être **visibles par les recherches** au plus tard 1 seconde après l'acquittement (par défaut), ou immédiatement avec l'option `consistency=strong`. | Essentielle | Test de lecture après écriture |
| EF-VEC-16 | **Instantanés** (*snapshots*) de collection exportables et importables. | Importante | Aller-retour export/import |

### 5.4 Objets (EF-OBJ)

| ID | Exigence | Priorité | Vérification |
|---|---|---|---|
| EF-OBJ-01 | **Compartiments** : `CreateBucket`, `DeleteBucket` (vide uniquement), `HeadBucket`, `ListBuckets`, `GetBucketLocation`. | Essentielle | Suite de compatibilité S3 |
| EF-OBJ-02 | **Objets** : `PutObject`, `GetObject`, `HeadObject`, `DeleteObject`, `DeleteObjects` (lot), `CopyObject`. | Essentielle | Suite de compatibilité S3 |
| EF-OBJ-03 | **Listage** : `ListObjectsV2` (préfixe, délimiteur, pagination par jeton, `StartAfter`), et `ListObjects` (V1) pour compatibilité. | Essentielle | Suite de compatibilité S3 |
| EF-OBJ-04 | **Multipart** : `CreateMultipartUpload`, `UploadPart`, `UploadPartCopy`, `CompleteMultipartUpload`, `AbortMultipartUpload`, `ListParts`, `ListMultipartUploads`. Objets jusqu'à **5 Tio**, parties de 5 Mio à 5 Gio, 10 000 parties maximum. | Essentielle | Envoi d'un objet de 100 Gio |
| EF-OBJ-05 | **Lectures par plage** (`Range`), requêtes conditionnelles (`If-Match`, `If-None-Match`, `If-Modified-Since`, `If-Unmodified-Since`), y compris écritures conditionnelles (`If-None-Match: *`). | Essentielle | Suite de compatibilité S3 |
| EF-OBJ-06 | **Authentification AWS Signature V4** (en-tête et requête présignée), y compris le mode *streaming* (`STREAMING-AWS4-HMAC-SHA256-PAYLOAD`) et `UNSIGNED-PAYLOAD`. | Essentielle | aws-cli, boto3, aws-sdk-js |
| EF-OBJ-07 | **URL présignées** pour GET et PUT avec durée de validité. | Essentielle | Téléchargement via URL présignée |
| EF-OBJ-08 | **Métadonnées utilisateur** (`x-amz-meta-*`), en-têtes standard (`Content-Type`, `Content-Encoding`, `Content-Disposition`, `Cache-Control`, `Expires`), **étiquettes** (`PutObjectTagging`, `GetObjectTagging`, `DeleteObjectTagging`). | Essentielle | Suite de compatibilité S3 |
| EF-OBJ-09 | **Sommes de contrôle** : ETag compatible S3 (MD5 pour objets simples, format multipart pour objets multiparties), et en-têtes `x-amz-checksum-*` (CRC32, CRC32C, SHA1, SHA256). | Essentielle | Vérification par les SDK |
| EF-OBJ-10 | **Adressage** par chemin (`/bucket/key`) et par hôte virtuel (`bucket.domaine/key`). | Essentielle | Les deux modes fonctionnent |
| EF-OBJ-11 | **Versionnement** des objets : `PutBucketVersioning`, `GetBucketVersioning`, `ListObjectVersions`, accès par `versionId`, marqueurs de suppression. | Importante | Suite de compatibilité S3 |
| EF-OBJ-12 | **Cycle de vie** : expiration et transition vers un niveau froid selon l'âge, le préfixe ou les étiquettes ; nettoyage des multiparties abandonnés. | Importante | Test de règle d'expiration |
| EF-OBJ-13 | **Politiques de compartiment** (sous-ensemble du langage de politique IAM) et ACL prédéfinies (`private`, `public-read`). | Importante | Tests d'autorisation |
| EF-OBJ-14 | **CORS** par compartiment. | Importante | Requête navigateur préliminaire (*preflight*) |
| EF-OBJ-15 | **Verrouillage d'objet** (*Object Lock*, rétention et conservation légale) en mode gouvernance et conformité. | Souhaitable | Suppression refusée pendant la rétention |
| EF-OBJ-16 | **Notifications d'événements** (création, suppression) vers un flux interne Ndaal (stream clé-valeur) ou un webhook. | Souhaitable | Événement reçu |
| EF-OBJ-17 | **Déduplication par contenu** optionnelle par compartiment (blocs identiques stockés une fois). | Souhaitable | Gain d'espace mesuré |

### 5.5 Transactions multi-modèles (EF-TX)

| ID | Exigence | Priorité | Vérification |
|---|---|---|---|
| EF-TX-01 | Ndaal DOIT permettre une **transaction atomique** regroupant des opérations clé-valeur, vectorielles et objet dans un même espace de noms : tout est appliqué, ou rien. Pour un espace de noms **réparti** (mode `sharded`, voir [ARCHITECTURE.md §5](ARCHITECTURE.md#5-espaces-de-noms-partitions-et-shards)), l'atomicité est garantie au sein d'un même **groupe de cohérence** (même clé de routage). | Essentielle | Injection de pannes à chaque étape : jamais d'état partiel |
| EF-TX-02 | Les transactions DOIVENT être **sérialisables** au sein d'un espace de noms (ou d'un groupe de cohérence). | Essentielle | Vérification d'historiques (voir tests) |
| EF-TX-03 | Une transaction DOIT pouvoir contenir des **préconditions** : existence ou absence d'une clé, valeur attendue, version d'objet (ETag), version de point. Si une précondition échoue, rien n'est appliqué et la raison est retournée. | Essentielle | Tests de préconditions |
| EF-TX-04 | Les **objets volumineux** d'une transaction DOIVENT pouvoir être téléversés **avant** la transaction (téléversement provisoire) puis rendus visibles par celle-ci. Un téléversement provisoire non validé DOIT être supprimé automatiquement après un délai configurable (24 h par défaut). | Essentielle | Test de nettoyage des orphelins |
| EF-TX-05 | Ndaal DOIT fournir des **liens** entre modèles : un point vectoriel ou une clé peut déclarer une référence vers un objet ; la suppression de l'objet PEUT déclencher la suppression en cascade des points et clés liés (option `cascade`). | Importante | UC-03 |
| EF-TX-06 | Ndaal DOIT fournir des **invalidations déclaratives** : une règle d'espace de noms peut associer un préfixe d'objet ou une collection à un motif de clés de cache invalidées à chaque modification. | Importante | Test d'invalidation |
| EF-TX-07 | La taille d'une transaction DOIT être limitée (par défaut 10 000 opérations et 64 Mio hors objets provisoires) et la limite DOIT être configurable. | Essentielle | Erreur explicite au-delà |
| EF-TX-08 | Le résultat d'une transaction DOIT renvoyer, pour chaque opération, son résultat individuel et le **numéro de séquence** (LSN) de validation. | Importante | API documentée |
| EF-TX-09 | Un client DOIT pouvoir demander une **lecture cohérente à un LSN** (lire ses propres écritures, même via un autre nœud). | Importante | Test multi-nœuds |
| EF-TX-10 | **Flux de changements** (*change data capture*) : un client peut s'abonner aux modifications d'un espace de noms depuis un LSN donné. | Souhaitable | Test de reprise depuis un LSN |

### 5.6 Stockage par niveaux (EF-NIV)

| ID | Exigence | Priorité | Vérification |
|---|---|---|---|
| EF-NIV-01 | Ndaal DOIT gérer trois niveaux : **RAM**, **NVMe local**, **stockage objet** (interne ou fournisseur S3 externe). | Essentielle | Configuration des trois niveaux |
| EF-NIV-02 | Le stockage objet DOIT pouvoir servir de **fondation durable** : en mode « désagrégé », toutes les données validées finissent dans le stockage objet, et un nœud peut être reconstruit uniquement à partir de celui-ci. | Essentielle | Destruction d'un nœud puis reconstruction |
| EF-NIV-03 | Les données DOIVENT migrer entre niveaux selon leur **fréquence d'accès** (promotion et rétrogradation automatiques). | Essentielle | Test de promotion après accès répétés |
| EF-NIV-04 | Des **règles de placement** DOIVENT permettre d'épingler un espace de noms, une collection, un préfixe de clé ou un compartiment sur un niveau (par exemple « toujours en RAM »). | Importante | Test d'épinglage |
| EF-NIV-05 | Les fournisseurs externes supportés DOIVENT inclure : S3 (AWS et compatibles), Google Cloud Storage, Azure Blob Storage, système de fichiers local. | Importante | Tests d'intégration par fournisseur |
| EF-NIV-06 | Le cache NVMe DOIT avoir une **taille maximale** configurable et ne jamais la dépasser. | Essentielle | Test de remplissage |

### 5.7 Réplication et distribution (EF-DIST)

| ID | Exigence | Priorité | Vérification |
|---|---|---|---|
| EF-DIST-01 | **Réplication synchrone** du journal avec facteur configurable (1, 3 ou 5), validation à la majorité (consensus Raft). | Essentielle | Perte d'un nœud sur trois sans perte de données |
| EF-DIST-02 | **Basculement automatique** du leader d'un groupe de partitions en cas de panne. | Essentielle | Temps de basculement mesuré (ENF-DISPO-02) |
| EF-DIST-03 | **Partitionnement** : les espaces de noms sont répartis en partitions logiques, regroupées en groupes de réplication placés sur les nœuds. | Essentielle | Cluster de 6 nœuds équilibré |
| EF-DIST-04 | **Rôles de nœuds** : `gateway` (frontaux de protocole sans état), `data` (journal et moteurs), `vector` (calcul vectoriel), `storage` (stockage objet interne), `all` (tous les rôles). Un nœud peut cumuler plusieurs rôles. | Essentielle | Cluster à rôles séparés |
| EF-DIST-05 | **Ajout et retrait de nœuds** à chaud avec **rééquilibrage** automatique et limitation de débit. | Essentielle | Ajout d'un nœud sous charge |
| EF-DIST-06 | **Routage transparent** : un client connecté à n'importe quel nœud accède à toutes les données (redirection interne). Le mode Redis Cluster (`CLUSTER SLOTS`, `CLUSTER SHARDS`, redirections `MOVED`/`ASK`) PEUT être activé pour les clients qui le supportent. | Essentielle | Clients simples et clients cluster |
| EF-DIST-07 | Stockage objet interne distribué avec **codes d'effacement** (Reed-Solomon, configurable, par exemple 4+2 ou 8+3) et réparation automatique. | Importante | Perte de 2 nœuds de stockage sur 6 sans perte |
| EF-DIST-08 | **Lectures sur réplicas** (*follower reads*) optionnelles avec borne d'obsolescence configurable. | Importante | Test de borne d'obsolescence |
| EF-DIST-09 | **Zones de disponibilité** : placement des réplicas sur des zones différentes (étiquettes de nœud). | Importante | Réplicas répartis sur 3 zones |
| EF-DIST-10 | **Mise à jour progressive** (*rolling upgrade*) d'un cluster sans interruption de service. | Essentielle | Mise à jour sous charge |

### 5.8 Sécurité (EF-SEC)

| ID | Exigence | Priorité | Vérification |
|---|---|---|---|
| EF-SEC-01 | **TLS 1.3** (et 1.2 configurable) sur tous les ports clients, et **mTLS** entre nœuds. | Essentielle | Analyse de configuration TLS |
| EF-SEC-02 | **Utilisateurs et clés d'accès** : identifiant + secret (S3), mot de passe ou jeton (RESP `AUTH`), jetons porteurs (HTTP). Les secrets DOIVENT être stockés hachés (Argon2id) sauf les secrets S3 qui DOIVENT être chiffrés (nécessaires au calcul de signature). | Essentielle | Revue de sécurité |
| EF-SEC-03 | **Contrôle d'accès** par rôle : permissions par espace de noms, par modèle (clé, vecteur, objet), par préfixe et par action (lecture, écriture, administration). Compatible avec les ACL Redis pour le sous-ensemble utile (`ACL SETUSER`, `ACL WHOAMI`, `ACL LIST`). | Essentielle | Matrice de tests d'autorisation |
| EF-SEC-04 | **Multi-locataire** : quotas par espace de noms (mémoire, disque, nombre de points, débit de requêtes, bande passante). | Importante | Dépassement de quota refusé |
| EF-SEC-05 | **Chiffrement au repos** des segments et du journal (AES-256-GCM), avec clé par espace de noms enveloppée par une clé maîtresse (fichier local ou KMS externe). | Importante | Données illisibles sans la clé |
| EF-SEC-06 | **Journal d'audit** des opérations d'administration et des accès refusés, exportable. | Importante | Entrées présentes et horodatées |
| EF-SEC-07 | **Rotation** des clés d'accès et des certificats sans redémarrage. | Importante | Rotation sous charge |
| EF-SEC-08 | Les entrées réseau DOIVENT être **validées et bornées** (taille de commande, nombre d'arguments, taille d'en-têtes, profondeur JSON) pour résister aux abus. | Essentielle | Campagne de fuzzing |

### 5.9 Exploitation (EF-OPS)

| ID | Exigence | Priorité | Vérification |
|---|---|---|---|
| EF-OPS-01 | **Métriques** au format Prometheus/OpenMetrics : latences par commande et par percentile, débit, mémoire par niveau, taille du journal, état de réplication, files d'attente par classe de charge. | Essentielle | Point `/metrics` |
| EF-OPS-02 | **Traces** distribuées OpenTelemetry (OTLP), avec échantillonnage configurable. | Importante | Trace visible dans un collecteur |
| EF-OPS-03 | **Journaux** structurés (JSON ou texte) avec niveaux configurables à chaud. | Essentielle | Changement de niveau sans redémarrage |
| EF-OPS-04 | **Sondes de santé** : vivacité (`/health/live`) et disponibilité (`/health/ready`). | Essentielle | Kubernetes les utilise |
| EF-OPS-05 | **Sauvegarde** complète et incrémentale (à partir d'un LSN) vers un stockage objet, cohérente entre les trois modèles. | Essentielle | Restauration vérifiée |
| EF-OPS-06 | **Restauration** complète et **à un instant donné** (*point-in-time*) dans la limite de rétention du journal. | Importante | Restauration à un LSN précis |
| EF-OPS-07 | **Images de conteneur** officielles (multi-architecture amd64 et arm64) et **chart Helm**. Images minimales construites en plusieurs étapes, exécutées sans privilèges (utilisateur non root, système de fichiers en lecture seule hors volume de données), avec contrôle de santé, signées et accompagnées d'une SBOM. | Essentielle | Déploiement Kubernetes de 3 nœuds ; analyse de vulnérabilités de l'image sans constat critique ni élevé |
| EF-OPS-08 | **Paquets** : binaires statiques Linux (musl, x86_64 et aarch64) pour la production ; macOS (aarch64 et x86_64) et Windows (x86_64) pour le développement et l'évaluation ; tous signés (Sigstore, plus notarisation Apple et Authenticode) et accompagnés de sommes de contrôle. | Importante | Binaires publiés et vérifiables sur les trois systèmes |
| EF-OPS-09 | **Commande de diagnostic** produisant une archive (configuration, métriques, journaux récents, état du cluster) sans secrets. | Importante | Archive générée |
| EF-OPS-10 | **Requêtes lentes** : journal des commandes dépassant un seuil (`SLOWLOG` côté RESP, équivalent HTTP). | Importante | Entrées présentes |
| EF-OPS-11 | **Console web embarquée** ([ADR-031](DECISIONS.md#adr-031--console-web-embarquée-en-svelte-et-typescript)) : servie par chaque nœud sur le port HTTP (`/console`), intégrée au binaire, désactivable. Écrans : tableau de bord (nœuds, partitions, métriques, alertes) ; explorateur de clés et console de commandes RESP ; explorateur de collections et terrain d'essai de recherche vectorielle (filtres, `explain`) ; explorateur de compartiments (envoi, téléchargement, métadonnées, versions) ; gestion des espaces de noms, utilisateurs, rôles, clés d'accès et quotas. Couverture fonctionnelle au moins équivalente aux consoles de Qdrant et de MinIO, et à RedisInsight pour l'exploration. | Importante | Scénarios de bout en bout dans un navigateur couvrant chaque écran |
| EF-OPS-12 | **Docker Compose** : fichiers fournis pour lancer en une commande un nœud unique, un cluster de 3 nœuds et l'environnement des bancs comparatifs (Ndaal, Valkey, Qdrant, MinIO). | Importante | `docker compose up` fonctionne sous Linux, macOS et Windows |
| EF-OPS-13 | **Publication automatisée** : une étiquette de version déclenche la construction, la signature et la publication des binaires, images, chart Helm, crates et notes de version, sans étape manuelle. | Essentielle | Publication de répétition (T14.11) entièrement automatique |
| EF-OPS-14 | **Exploration multi-modèles dans la console** : explorateur des liens entre objets, vecteurs et clés (dans les deux sens) ; éditeur de transactions multi-modèles avec simulation (`dry_run`) avant exécution. | Importante | Scénario UC-01 construit, simulé et exécuté depuis la console |

### 5.10 Outils et SDK (EF-OUT)

| ID | Exigence | Priorité | Vérification |
|---|---|---|---|
| EF-OUT-01 | **SDK Rust** officiel : clé-valeur, vecteurs, objets, transactions, administration. | Essentielle | Exemples compilés et testés |
| EF-OUT-02 | **SDK Python** officiel (synchrone et asynchrone). | Importante | Publié sur PyPI, tests |
| EF-OUT-03 | **SDK TypeScript** officiel (Node.js et navigateur pour les URL présignées). | Importante | Publié sur npm, tests |
| EF-OUT-04 | **Outil de migration** depuis Redis (lecture RDB ou réplication), Qdrant (instantanés ou API) et S3/MinIO (copie de compartiments). | Importante | Migration d'un jeu de test |
| EF-OUT-05 | **Banc d'essai intégré** `ndaal bench` pour les trois modèles et les charges mixtes. | Importante | Rapport généré |

---

## 6. Exigences non fonctionnelles

### 6.1 Performance (ENF-PERF)

Toutes les valeurs s'entendent sur l'[environnement de référence](#7-environnement-de-référence), client sur une machine séparée du même réseau, sauf mention contraire. Ce sont des **objectifs à atteindre et à mesurer**, pas des garanties.

Ces valeurs absolues sont des **planchers**. La cible réelle est la parité avec les services remplacés ([§6.12, ENF-COMP](#612-parité-avec-les-services-remplacés-enf-comp)) : lorsque les deux s'appliquent, la plus exigeante l'emporte.

| ID | Exigence | Cible |
|---|---|---|
| ENF-PERF-01 | Latence `GET`/`SET` clé-valeur (valeurs de 100 octets, clés en RAM, mode non répliqué) | p50 < 150 µs, p99 < 500 µs, p99,9 < 2 ms (côté serveur : p99 < 50 µs) |
| ENF-PERF-02 | Débit clé-valeur | ≥ 150 000 opérations/s **par cœur** dédié au clé-valeur, croissance quasi linéaire jusqu'à 16 cœurs |
| ENF-PERF-03 | Latence d'écriture durable répliquée (3 réplicas, même zone) | p99 < 3 ms |
| ENF-PERF-04 | Recherche vectorielle, 1 M de vecteurs de dimension 768, `f32`, cosinus, index HNSW en RAM, top-10 | Rappel@10 ≥ 0,95 avec p99 < 5 ms et ≥ 1 000 requêtes/s sur 8 cœurs vectoriels (plancher ; cible : ENF-COMP-04) |
| ENF-PERF-05 | Recherche vectorielle avec index sur disque, 100 M de vecteurs de dimension 768 | Rappel@10 ≥ 0,90 avec p99 < 50 ms |
| ENF-PERF-06 | Ingestion vectorielle | ≥ 20 000 points/s (dimension 768) par nœud, index construit en arrière-plan |
| ENF-PERF-07 | Débit objet, gros objets (≥ 64 Mio) | ≥ 80 % du débit séquentiel des disques ou du réseau (le plus faible des deux) |
| ENF-PERF-08 | Latence objet, petits objets (≤ 64 Kio) | p99 < 2 ms en lecture depuis le cache NVMe, p99 < 1 ms pour les objets en ligne (≤ 64 Kio, servis depuis la RAM) |
| ENF-PERF-09 | Démarrage à froid d'un nœud avec 10 Gio de journal non compacté | < 60 s jusqu'à l'état « prêt » |
| ENF-PERF-10 | Surcoût d'une transaction multi-modèles par rapport aux mêmes opérations séparées | < 20 % de latence supplémentaire |
| ENF-PERF-11 | Débit clé-valeur en pipeline (profondeur 16, valeurs de 100 octets) | ≥ 1 000 000 opérations/s **par cœur** dédié au clé-valeur |

### 6.2 Isolation des charges (ENF-ISO)

| ID | Exigence | Cible |
|---|---|---|
| ENF-ISO-01 | Latence p99 du clé-valeur pendant un transfert objet saturant les disques | Dégradation < 20 % par rapport au repos |
| ENF-ISO-02 | Latence p99 du clé-valeur pendant la construction d'un index vectoriel | Dégradation < 20 % |
| ENF-ISO-03 | Latence p99 de recherche vectorielle pendant une ingestion massive | Dégradation < 50 % |
| ENF-ISO-04 | Un locataire saturant son quota NE DOIT PAS dégrader les autres locataires de plus de 10 % | Test multi-locataire |
| ENF-ISO-05 | En surcharge, Ndaal DOIT **refuser** des requêtes (erreur explicite de type « occupé », `503 SlowDown` en S3) plutôt que de laisser croître les files sans limite | Mémoire bornée sous surcharge |

### 6.3 Durabilité et cohérence (ENF-DUR)

| ID | Exigence | Cible |
|---|---|---|
| ENF-DUR-01 | Aucune écriture acquittée en mode durable ne DOIT être perdue après un arrêt brutal (coupure de courant) d'un nœud unique | 0 perte sur 10 000 coupures simulées |
| ENF-DUR-02 | En mode répliqué, aucune écriture acquittée ne DOIT être perdue tant qu'une majorité de réplicas survit | 0 perte en simulation déterministe |
| ENF-DUR-03 | Toutes les données sur disque DOIVENT être protégées par une **somme de contrôle** ; une corruption DOIT être détectée et signalée, jamais servie | Test d'injection de corruption |
| ENF-DUR-04 | Les modes de durabilité DOIVENT être configurables : `none` (volatile), `async` (vidage périodique, par défaut 100 ms), `sync` (vidage avant acquittement) | Tests par mode |
| ENF-DUR-05 | Durabilité du stockage objet interne distribué : conception visant au moins 11 neuf annuels avec un code d'effacement 8+3 sur des domaines de panne indépendants | Calcul documenté |
| ENF-DUR-06 | Les lectures d'un client DOIVENT refléter ses propres écritures (*read-your-writes*) par défaut | Test de session |

### 6.4 Disponibilité (ENF-DISPO)

| ID | Exigence | Cible |
|---|---|---|
| ENF-DISPO-01 | Disponibilité d'un cluster de 3 nœuds sur 3 zones | Conception visant 99,95 % |
| ENF-DISPO-02 | Temps de basculement après panne d'un leader | < 5 s (détection + élection), < 10 s au p99 |
| ENF-DISPO-03 | Mise à jour progressive sans erreur côté client (hors reconnexions transparentes) | 0 erreur non réessayable |
| ENF-DISPO-04 | Opérations de maintenance (compactage, rééquilibrage, réparation) sans interruption de service | Vérifié sous charge |

### 6.5 Passage à l'échelle (ENF-SCAL)

| ID | Exigence | Cible |
|---|---|---|
| ENF-SCAL-01 | Nombre de nœuds par cluster | Jusqu'à 128 nœuds testés |
| ENF-SCAL-02 | Nombre d'espaces de noms | ≥ 100 000 par cluster |
| ENF-SCAL-03 | Nombre de clés | ≥ 1 milliard par nœud (avec débordement NVMe) |
| ENF-SCAL-04 | Nombre de points vectoriels | ≥ 1 milliard par cluster |
| ENF-SCAL-05 | Nombre d'objets | ≥ 10 milliards par cluster ; ≥ 100 millions par compartiment avec listage paginé efficace |
| ENF-SCAL-06 | Débit agrégé | Croissance quasi linéaire (≥ 80 % d'efficacité) jusqu'à 32 nœuds |

### 6.6 Ressources (ENF-RES)

| ID | Exigence | Cible |
|---|---|---|
| ENF-RES-01 | Empreinte mémoire au repos d'un nœud vide | < 100 Mio |
| ENF-RES-02 | Fonctionnement sur petite machine (2 cœurs, 2 Gio de RAM) en mode nœud unique | Démarrage et charge légère sans erreur |
| ENF-RES-03 | Taille du binaire statique | < 100 Mio |
| ENF-RES-04 | La mémoire totale utilisée DOIT rester sous la limite configurée (`memory.limit`), y compris en surcharge | Test de surcharge |
| ENF-RES-05 | Surcoût mémoire par clé (petite clé, petite valeur) | < 64 octets hors clé et valeur |

### 6.7 Sécurité (ENF-SEC)

| ID | Exigence | Cible |
|---|---|---|
| ENF-SEC-01 | Le code `unsafe` DOIT être limité à des modules identifiés, documentés (justification `// SAFETY:`) et couverts par Miri ou des tests dédiés | Inventaire automatique en CI |
| ENF-SEC-02 | Les dépendances DOIVENT être auditées (`cargo audit`, `cargo deny`) à chaque intégration | CI bloquante |
| ENF-SEC-03 | Les décodeurs de protocoles et de formats DOIVENT être fuzzés en continu | ≥ 1 milliard d'exécutions cumulées par décodeur avant publication stable |
| ENF-SEC-04 | Configuration sûre par défaut : écoute sur `127.0.0.1` sans authentification configurée ; refus d'écouter sur une interface publique sans authentification sauf option explicite | Test de démarrage |
| ENF-SEC-05 | Processus de signalement de vulnérabilités documenté (`SECURITY.md`) | Fichier présent |

### 6.8 Qualité du code (ENF-QUAL)

| ID | Exigence | Cible |
|---|---|---|
| ENF-QUAL-01 | Couverture de tests des crates du moteur de stockage | ≥ 85 % des lignes ; ≥ 70 % globalement |
| ENF-QUAL-02 | `cargo clippy` sans avertissement, `rustfmt` appliqué | CI bloquante |
| ENF-QUAL-03 | Chaque format sur disque et chaque message réseau DOIT être versionné | Revue |
| ENF-QUAL-04 | Toute correction de bogue DOIT être accompagnée d'un test qui échouait avant la correction | Revue |
| ENF-QUAL-05 | La simulation déterministe DOIT tourner en continu en CI | ≥ 10 000 graines par jour |
| ENF-QUAL-06 | CI/CD sur **GitHub Actions** ([ADR-028](DECISIONS.md#adr-028--cicd-sur-github-actions)) : toutes les vérifications de [STRATEGIE_DE_TESTS §13](STRATEGIE_DE_TESTS.md#13-intégration-continue) sont automatisées ; branche principale protégée (CI verte et relecture obligatoires) | CI bloquante |
| ENF-QUAL-07 | Sécurité de la chaîne de production : actions tierces épinglées par empreinte de commit, permissions minimales des jetons, mises à jour automatiques des dépendances (Dependabot), attestations de provenance des artefacts (SLSA) | Revue + `cargo deny` |

### 6.9 Portabilité (ENF-PORT)

| ID | Exigence | Cible |
|---|---|---|
| ENF-PORT-01 | Plateformes de production | Linux x86_64 et aarch64, noyau ≥ 5.10 |
| ENF-PORT-02 | Plateformes de développement et d'évaluation, de **premier rang** | macOS (aarch64, x86_64) et Windows (x86_64) via les implémentations d'E/S de repli : **toute** la suite de tests (hors bancs de performance et pannes réelles) passe en CI sur ces systèmes |
| ENF-PORT-03 | Systèmes de fichiers supportés en production | ext4, XFS |
| ENF-PORT-04 | Version minimale de Rust (MSRV) | Déclarée dans `Cargo.toml`, vérifiée en CI, révisée au plus tous les 6 mois |
| ENF-PORT-05 | Performance hors Linux | Les objectifs ENF-PERF et ENF-COMP ne s'appliquent qu'à Linux. Sous macOS et Windows, Ndaal DOIT fonctionner correctement ; ses performances y sont mesurées et documentées, sans être garanties |

### 6.10 Compatibilité (ENF-COMPAT)

| ID | Exigence | Cible |
|---|---|---|
| ENF-COMPAT-01 | Clients Redis | redis-cli, redis-py, ioredis, node-redis, go-redis, Jedis, Lettuce, redis-rs : suites de tests de base réussies |
| ENF-COMPAT-02 | Clients S3 | aws-cli, boto3, aws-sdk-js v3, aws-sdk-go v2, rclone, s3cmd, MinIO client (`mc`) : opérations du périmètre réussies |
| ENF-COMPAT-03 | Suite de conformité S3 | ≥ 90 % de réussite des tests de la suite `ceph/s3-tests` restreinte au périmètre déclaré |
| ENF-COMPAT-04 | Compatibilité ascendante des formats | Un nœud peut lire les données écrites par les deux publications majeures précédentes |
| ENF-COMPAT-05 | Compatibilité de protocole entre nœuds | Mise à jour progressive possible entre deux publications consécutives |

### 6.11 Documentation (ENF-DOC)

| ID | Exigence | Cible |
|---|---|---|
| ENF-DOC-01 | Documentation utilisateur : démarrage rapide, guides par modèle, référence de configuration, référence API | Publiée avec chaque version |
| ENF-DOC-02 | Documentation de référence générée depuis le code (`rustdoc`) pour les crates publiques | 100 % des éléments publics documentés |
| ENF-DOC-03 | Liste exhaustive des commandes Redis et opérations S3 supportées, générée automatiquement | Tableau à jour |

### 6.12 Parité avec les services remplacés (ENF-COMP)

Ndaal n'a d'intérêt que s'il ne fait **rien perdre** par rapport aux services qu'il remplace. Chaque mesure est faite **côte à côte** avec le service de référence, sur la même machine, avec le même client, la même charge et une **durabilité équivalente** (voir [ADR-027](DECISIONS.md#adr-027--parité-de-performance-avec-les-services-remplacés)).

**Règle** : pour chaque banc, Ndaal DOIT atteindre au moins **95 %** du débit de la référence et au plus **105 %** de sa latence p99 (tolérance de bruit de mesure). Ndaal DEVRAIT **dépasser** la référence. Tout écart défavorable au-delà de la tolérance est un défaut bloquant pour la publication.

**Références** : dernière version stable publiée au moment de la mesure, configuration recommandée par son éditeur pour la production ; versions et configurations consignées avec les résultats.

| ID | Mesure | Référence | Équivalence imposée |
|---|---|---|---|
| ENF-COMP-01 | Latence et débit `GET`/`SET`, valeurs de 100 octets et 1 Kio, ratio 1:10 et 1:1, sans pipeline | Valkey | Même budget CPU : Ndaal limité à N cœurs contre Valkey avec N `io-threads`, pour N = 1, 4, 16 ; durabilité `none`/`async`/`sync` contre `appendonly no`/`everysec`/`always` |
| ENF-COMP-02 | Débit en pipeline (profondeur 16 et 64) et sur hash, liste, ensemble trié | Valkey | Idem ENF-COMP-01 |
| ENF-COMP-03 | Débit machine entière (16 cœurs) | Valkey en cluster de 16 instances sur la même machine | Ndaal en nœud unique 16 cœurs, client compatible cluster pour Valkey |
| ENF-COMP-04 | Recherche vectorielle HNSW en RAM : débit à rappel@10 = 0,95 et 0,99, latence p99 ; 1 M × 768 et 10 M × 768, avec et sans filtre (sélectivité 50 %, 5 %, 0,5 %) | Qdrant | Mêmes `m` et `ef_construct`, même quantification (aucune, puis int8), même nombre de cœurs ; comparaison à **rappel égal**, jamais à paramètres égaux seuls |
| ENF-COMP-05 | Ingestion vectorielle (points/s jusqu'à index prêt) et empreinte mémoire | Qdrant | Mêmes paramètres d'index et mêmes lots |
| ENF-COMP-06 | Débit objet gros objets (64 Mio–1 Gio), `PUT` et `GET` | MinIO (ou RustFS si MinIO n'est plus disponible en édition libre) | Mêmes disques, même nœud unique, vérification d'intégrité active des deux côtés ; puis 4 nœuds avec code d'effacement de même géométrie |
| ENF-COMP-07 | Débit et latence p99 petits objets (4–64 Kio), `PUT`, `GET`, `LIST` | MinIO ou RustFS | Idem ENF-COMP-06, `fsync` avant acquittement des deux côtés |
| ENF-COMP-08 | Charge mixte « RAG » (UC-01/UC-02) : Ndaal seul contre Valkey + Qdrant + MinIO sur la même machine | Les trois ensemble | Ressources totales identiques ; Ndaal DOIT être meilleur (débit et p99), pas seulement à parité, puisqu'il économise les allers-retours entre services |

### 6.13 Internationalisation (ENF-I18N)

Seules les interfaces **lues par un humain** sont traduites. Les interfaces **lues par une machine** restent en anglais et stables, car les clients existants les analysent ([ADR-029](DECISIONS.md#adr-029--internationalisation-limitée-aux-interfaces-humaines)).

| ID | Exigence | Cible |
|---|---|---|
| ENF-I18N-01 | Interfaces traduites : console web, messages de la CLI `ndaal` (aide, erreurs, invites, tableaux), documentation utilisateur | Français et anglais complets à la première publication stable |
| ENF-I18N-02 | Interfaces **jamais** traduites : messages et codes d'erreur des protocoles RESP, S3, HTTP et gRPC, journaux, métriques, noms des paramètres de configuration, sortie `--output json` | Anglais stable, vérifié par les tests de compatibilité |
| ENF-I18N-03 | Choix de la langue, par ordre de priorité : option `--lang`, variable `NDAAL_LANG`, paramètre `node.lang`, langue du système (`LC_ALL`/`LC_MESSAGES`/`LANG`, préférences macOS et Windows), puis anglais. Console : préférence de l'utilisateur, puis en-tête `Accept-Language` | Tests par plateforme |
| ENF-I18N-04 | Traductions dans des fichiers séparés (Fluent `.ftl`) ; une clé absente dans une langue retombe sur l'anglais, sans jamais provoquer d'erreur | Test de repli |
| ENF-I18N-05 | Complétude vérifiée en CI : chaque clé existe dans chaque langue livrée ; aucun texte destiné à un humain n'est écrit en dur dans la CLI ou la console | CI bloquante |
| ENF-I18N-06 | Formats régionaux dans les interfaces humaines (nombres, dates, durées) ; formats machine toujours neutres (ISO 8601, point décimal, unités binaires `MiB`) | Tests |

### 6.14 Console web (ENF-UI)

| ID | Exigence | Cible |
|---|---|---|
| ENF-UI-01 | Accessibilité : WCAG 2.1 niveau AA, navigation complète au clavier, contrastes suffisants en thème clair et sombre | Analyse automatique (axe) sans violation ; revue manuelle avant publication stable |
| ENF-UI-02 | Navigateurs : deux dernières versions de Chrome/Edge, Firefox et Safari | Tests de bout en bout sur les trois moteurs (Chromium, Gecko, WebKit) |
| ENF-UI-03 | Performance : chargement initial < 2 s sur réseau local ; fichiers de la console < 1 Mio compressés ; listes paginées ou virtualisées, jamais de chargement complet d'une collection, d'un compartiment ou d'un espace de clés | Budget vérifié en CI |
| ENF-UI-04 | Sécurité : session par cookie `HttpOnly`, `Secure`, `SameSite=Strict` ; jeton anti-CSRF ; politique de sécurité du contenu (CSP) stricte sans script en ligne ; interdiction d'affichage dans un cadre ; expiration après 30 min d'inactivité (configurable) ; confirmation explicite des actions destructrices | Tests de sécurité et matrice d'autorisation |
| ENF-UI-05 | Aucune fonction réservée à la console : chaque action passe par l'API HTTP publique documentée, avec les mêmes contrôles d'autorisation | Revue ; tests de la matrice d'autorisation |
| ENF-UI-06 | Dépendances JavaScript minimales, versions verrouillées, auditées en CI ; licences conformes à C-02 | CI bloquante |
| ENF-UI-07 | La console intégrée ajoute moins de 5 Mio au binaire (ENF-RES-03 reste respectée) ; le binaire se construit sans Node.js lorsque la fonctionnalité `console` est désactivée | CI |

---

## 7. Environnement de référence

Machine utilisée pour toutes les mesures de performance, sauf mention contraire :

| Élément | Valeur |
|---|---|
| CPU | 16 cœurs physiques x86_64 (classe AMD EPYC ou Intel Xeon récents), SMT désactivé pour les mesures |
| RAM | 64 Gio |
| Disques | 2 × NVMe (≥ 3 Gio/s en lecture séquentielle chacun) |
| Réseau | 25 Gbit/s |
| Système | Linux, noyau ≥ 6.1, système de fichiers XFS |
| Client | Machine identique, même commutateur réseau |

Une configuration « petite machine » (2 cœurs, 2 Gio de RAM, SSD SATA) sert à vérifier ENF-RES-02.

---

## 8. Contraintes

| ID | Contrainte |
|---|---|
| C-01 | Le code du serveur DOIT être écrit en **Rust** (édition 2021 ou ultérieure). Aucune dépendance native en C/C++ dans le chemin critique, à l'exception de la bibliothèque C standard, des appels système et des primitives cryptographiques de `ring` (voir [ADR-020](DECISIONS.md#adr-020--exception-cryptographique-ring)). Les dépendances C optionnelles (par exemple compression) DOIVENT avoir une alternative en Rust pur. La console web, exécutée dans le navigateur, est écrite en TypeScript ([ADR-031](DECISIONS.md#adr-031--console-web-embarquée-en-svelte-et-typescript)) ; elle n'est jamais nécessaire au fonctionnement du serveur. |
| C-02 | Licence du projet : **Apache 2.0**. Dépendances autorisées : MIT, Apache-2.0, BSD-2/3, ISC, Zlib, Unicode-DFS, Unicode-3.0 (sa remplaçante, ajoutée le 2026-10-13), MPL-2.0 (sans modification). Interdites : GPL, AGPL, SSPL, BSL et licences non libres. Contrôlé par `cargo deny`. |
| C-03 | Aucune dépendance à un service externe pour fonctionner en mode nœud unique (pas de ZooKeeper, etcd, base externe). |
| C-04 | Les protocoles Redis et S3 sont implémentés à partir de leur documentation publique ; aucun code sous licence incompatible n'est réutilisé. |
| C-05 | Les marques Redis, Amazon S3, Qdrant, MinIO sont citées à titre de compatibilité uniquement. |

---

## 9. Hypothèses

| ID | Hypothèse | Impact si fausse |
|---|---|---|
| H-01 | Les embeddings sont calculés hors de Ndaal | Ajouter une extension d'inférence |
| H-02 | La majorité des déploiements tient sur 1 à 10 nœuds | Prioriser davantage le passage à l'échelle massif |
| H-03 | Les disques de production sont des SSD NVMe | Prévoir des réglages pour disques rotatifs |
| H-04 | Les clients acceptent un sous-ensemble documenté de Redis et S3 | Élargir la compatibilité |
| H-05 | Les transactions multi-modèles concernent un seul espace de noms dans la grande majorité des cas | Étudier les transactions inter-espaces |
| H-06 | Le noyau Linux cible supporte `io_uring` | Utiliser le repli `epoll` + pool de threads (déjà prévu) |

---

## 10. Risques

| ID | Risque | Probabilité | Impact | Atténuation |
|---|---|---|---|---|
| R-01 | **Ampleur du projet** : trois systèmes complexes en un | Élevée | Élevé | Découpage en jalons livrant chacun un produit utilisable ; réutilisation de crates éprouvées ; nœud unique d'abord |
| R-02 | **Bogues de durabilité** (perte ou corruption de données) | Moyenne | Critique | Simulation déterministe dès le Jalon 2, sommes de contrôle partout, fuzzing, campagnes de pannes |
| R-03 | **Performance inférieure à un service remplacé** (Redis/Valkey, Qdrant, MinIO) | Moyenne | Critique | Architecture thread-per-core, bancs comparatifs ENF-COMP côte à côte en CI nocturne dès le premier banc, porte bloquante à chaque jalon concerné, profilage continu, budget de latence par couche |
| R-04 | **Compatibilité incomplète** avec les clients Redis/S3 | Élevée | Moyen | Suites de compatibilité automatisées avec les vrais clients ; liste publique du supporté |
| R-05 | **Contention** malgré l'isolation (cache CPU, bande passante mémoire, contrôleur NVMe) | Moyenne | Élevé | Mesures ENF-ISO en CI ; ordonnanceur d'E/S à priorités ; limitation de débit des tâches de fond |
| R-06 | **Complexité de Raft multi-groupes** | Moyenne | Élevé | Utilisation d'`openraft`, simulation déterministe réseau, tests de type Jepsen |
| R-07 | **Rappel vectoriel dégradé** avec filtres sélectifs ou index sur disque | Moyenne | Moyen | Planificateur de requête filtrée, bancs ANN publics, réglages adaptatifs |
| R-08 | **Dépendances** abandonnées ou au changement de licence | Faible | Moyen | `cargo deny`, abstraction légère des composants critiques, possibilité de fork |
| R-09 | **Concurrence** : écosystème en évolution rapide | Élevée | Moyen | Miser sur la différence (transactions multi-modèles, binaire unique) |
| R-10 | **Sécurité** : surface d'attaque de trois protocoles | Moyenne | Élevé | Fuzzing, validation stricte, audit externe avant publication stable |

---

## 11. Critères d'acceptation globaux

Le produit est considéré **prêt pour la production** lorsque :

1. Toutes les exigences **Essentielles** sont satisfaites et vérifiées.
2. Au moins 80 % des exigences **Importantes** sont satisfaites ; les autres sont documentées comme limitations connues.
3. Les objectifs ENF-PERF-01 à 04, ENF-PERF-07, ENF-PERF-11, ENF-ISO-01 et ENF-ISO-02 sont atteints et publiés avec la méthode de mesure.
4. **Toutes** les exigences de parité ENF-COMP-01 à 08 sont atteintes et publiées avec les versions, configurations et scripts permettant de les reproduire. Aucune dérogation n'est possible.
5. Les campagnes de pannes (ENF-DUR-01, ENF-DUR-02) n'ont révélé aucune perte de données acquittées sur la dernière série complète.
6. Les suites de compatibilité (ENF-COMPAT-01 à 03) atteignent leurs seuils.
7. Un audit de sécurité externe a été réalisé et ses constats critiques et élevés sont corrigés.
8. L'application de référence RAG (UC-01, UC-02, UC-03) fonctionne de bout en bout sur un cluster de 3 nœuds.
9. La documentation utilisateur (ENF-DOC-01) est complète.

---

## 12. Matrice de traçabilité exigences → jalons

Les jalons sont définis dans [CAHIER_DES_TACHES.md](CAHIER_DES_TACHES.md).

| Famille | Jalons principaux |
|---|---|
| EF-GEN | J01, J02, J12 |
| EF-KV-01 à 12, 15, 16 | J03, J07 |
| EF-KV-13, 14, 17 | J13 |
| EF-VEC-01 à 10, 15, 16 | J05 |
| EF-VEC-11 à 14 | J13 |
| EF-OBJ-01 à 10 | J04 |
| EF-OBJ-11 à 17 | J13 |
| EF-TX-01 à 08 | J06 |
| EF-TX-09, 10 | J09, J13 |
| EF-NIV | J08 |
| EF-DIST-01, 02, 08 | J09 |
| EF-DIST-03 à 07, 09, 10 | J10 |
| EF-SEC-01 à 03, 08 | J03, J04, J11 |
| EF-SEC-04 à 07 | J11 |
| EF-OPS-01, 03, 04 | J01, J02, J12 |
| EF-OPS-02, 05 à 13 | J12 (premières versions d'EF-OPS-12 et 13 en J01) |
| EF-OPS-14 | J13 |
| EF-OUT-01 | J06, J13 |
| EF-OUT-02 à 05 | J13 |
| ENF-PERF, ENF-ISO, ENF-RES | J07, puis vérification J14 |
| ENF-COMP-01 à 03 | J03 (mesure), J07 (atteinte), vérification J14 |
| ENF-COMP-04, 05 | J05 (mesure), J07 (atteinte), vérification J14 |
| ENF-COMP-06, 07 | J04 (mesure), J07 (atteinte), J10 (code d'effacement), vérification J14 |
| ENF-COMP-08 | J06 (mesure), J07 (atteinte), vérification J14 |
| ENF-DUR | J02, J09, vérification J14 |
| ENF-DISPO, ENF-SCAL | J09, J10, vérification J14 |
| ENF-SEC | J01 (outillage), J11, J14 |
| ENF-QUAL | J01 (outillage), continu |
| ENF-PORT | J01 (CI multi-plateforme), J12 (binaires) |
| ENF-I18N | J01 (fondations), J12 (traductions complètes) |
| ENF-UI | J12 |
| ENF-COMPAT | J03, J04, J14 |
| ENF-DOC | Continu, finalisation J14 |

La correspondance détaillée tâche par tâche figure dans le cahier des tâches (colonne « Exigences »).
