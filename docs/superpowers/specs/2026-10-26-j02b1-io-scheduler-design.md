# J02-B1 — Ordonnanceur d'E/S et pool de threads : spécification

**Tâches :** T02.05, T02.06 ([CAHIER_DES_TACHES.md](../../../CAHIER_DES_TACHES.md#j02--noyau-de-stockage)).

**Exigences :** ENF-ISO-01, ADR-016, ADR-017, ADR-003, ENF-PORT-03, P8.

**Sources :**
- [ARCHITECTURE §4.3](../../../ARCHITECTURE.md#43-ordonnanceur-des) ;
- [EXPLOITATION §3](../../../EXPLOITATION.md#3-référence-de-configuration) (`[io]`) ;
- [DECISIONS](../../../DECISIONS.md) ADR-016.

**Statut :** conception validée par le responsable du projet le 2026-10-26, section par section.

## 1. Contexte et périmètre

J02-B a été découpé en deux, avec l'accord du responsable. **B1 (ce document) :** l'ordonnanceur d'E/S et le pool de threads, dont le journal (C) a besoin. **B2 :** la priorité sur les cœurs partagés (T02.04), plus tard.

**Hors du périmètre :**
- T02.04 ;
- `io_uring` et `O_DIRECT` (J07) ;
- les disques multiples `[[disks]]` (J08) : un seul périphérique, le dossier de données, mais l'interface prévoit un ordonnanceur par périphérique ;
- la réduction automatique du débit de fond selon la latence des lectures hautes (J07, point reporté).

## 2. Décisions

| # | Décision | Raison |
|---|---|---|
| 1 | La priorité se donne **à l'ouverture d'un fichier** : toutes les opérations du descripteur la portent. Deux sous-systèmes de priorités différentes ouvrent chacun le fichier. | Le trait `File` ne change pas, et l'API reste simple |
| 2 | **Pool de threads dédié** par périphérique, de `io.queue_depth` threads, placés sur les cœurs de classe `io` | Maîtrise du nombre de threads et de leur placement ; aucun partage avec `spawn_blocking` |
| 3 | **Seaux à jetons avec bande passante mesurée**, sans réglage automatique selon la latence | Fidèle à « bande passante mesurée » ; le réglage se fera avec les bancs d'isolation de J07 |
| 4 | La logique d'ordonnancement est une **machine à états pure** (temps passé en paramètre), commune au réel et à la simulation | Le critère de T02.05 se vérifie en simulation, de façon déterministe |

## 3. Composants (dans `ndaal-io`)

### 3.1 `sched::Scheduler` : un périphérique

| Élément | Règle |
|---|---|
| Classes | `Critical` (journal), `High` (lectures du chemin court), `Medium` (vectoriel), `Low` (transferts), `Background` (compactage) |
| Critique | Toujours servie la première |
| Autres classes | Tourniquet à déficit en octets, quantums proportionnels à 50/25/15/10. Une opération sans taille (`sync_data`, `allocate`) compte pour 4 Kio |
| Fond | Seau à jetons : débit = `io.background_bandwidth_share` × bande passante mesurée, capacité = 100 ms de ce débit. Sans jetons, le fond attend même si le périphérique est libre |
| Mesure | Moyenne glissante des octets servis, sur des fenêtres de 100 ms où le périphérique est occupé ; valeur de départ de 100 Mio/s |
| Profondeur | Au plus `io.queue_depth` opérations en cours |

Interface :
- `submit(op)` ;
- `next(now) -> Option<op>` ;
- `complete(op, bytes, now)` ;
- `next_wakeup(now) -> Option<Instant>`, qui donne le moment où des jetons seront disponibles.

Les données de chaque opération restent chez l'appelant ; le `Scheduler` ne manipule que des descripteurs (classe, taille, numéro).

### 3.2 `ScheduledDisk<D>`

`open(path, options, priority)` rend un fichier dont toutes les opérations passent par l'ordonnanceur du périphérique. Les autres méthodes de `Disk` (renommer, supprimer, lister, synchroniser un dossier) passent aussi par l'ordonnanceur, en classe `Critical` : elles sont rares, et le journal et le manifeste en dépendent.

### 3.3 `real::ThreadPool` (T02.06)

- Un **répartiteur** par périphérique tient le `Scheduler`, lit les demandes des cœurs et confie les opérations à `io.queue_depth` **threads d'E/S**, placés sur les cœurs `io`.
- **Pas de verrou partagé sur le chemin des cœurs** (ADR-003). Chaque cœur dépose ses demandes dans une file bornée sans verrou (`rtrb`) lue par le seul répartiteur. Une file pleine donne `io::ErrorKind::WouldBlock` (« occupé », P8).
- Les threads exécutent `pread`, `pwrite`, `fdatasync`, `fallocate` (Linux) ou leur équivalent, puis réveillent le cœur demandeur.
- **Arrêt :** les opérations en attente échouent avec `Interrupted`, celles en cours se terminent, puis les threads sont rejoints, en 10 s au plus.

### 3.4 `sim::SimDevice`

- Un temps de service par opération : latence fixe plus taille divisée par le débit, avec une variation tirée de la graine. La même profondeur bornée qu'en réel.
- Il entoure le `SimDisk` existant, qui garde le contenu et les pannes.

### 3.5 Nouvelle opération `File::allocate(len)`

Elle réserve de l'espace sans changer le contenu : `fallocate` sous Linux, et `set_len` vers le haut ailleurs si la taille est plus petite. Elle est ajoutée au trait, aux disques réel et simulé, et au contrat commun.

### 3.6 Configuration

Documentée dans EXPLOITATION §3.

| Clé | Type | Défaut |
|---|---|---|
| `io.backend` | `auto` ou `threads` (`uring` en J07) | `auto` (aujourd'hui : `threads`) |
| `io.queue_depth` | entier de 1 à 1024 | `32` |
| `io.background_bandwidth_share` | ratio de 0 à 1 (nouveau type) | `0.30` |

`io.direct_io` reste réservé à `io_uring` (J07).

### 3.7 Métriques

| Nom | Type | Étiquettes |
|---|---|---|
| `ndaal_io_ops_total` | compteur | `device`, `class`, `op` |
| `ndaal_io_latency_seconds` | histogramme (attente et service) | `device`, `class` |
| `ndaal_io_inflight` | jauge | `device` |
| `ndaal_io_bandwidth_bytes_per_second` | jauge | `device` |
| `ndaal_io_throttled_total` | compteur (attentes du fond faute de jetons) | `device` |

## 4. Tests et critères de fin

| Critère | Preuve |
|---|---|
| T02.05 : en simulation, une rafale d'écritures `basse` ne retarde pas une lecture `haute` au-delà d'une borne configurée | Scénario `io_priority` dans `ndaal-sim`, sur un `SimDevice` à 200 Mio/s et 100 µs de latence. Une rafale continue d'écritures `basse` de 1 Mio occupe le périphérique, et une lecture `haute` de 4 Kio part toutes les 1 ms. La latence p99 des lectures hautes reste sous la borne profondeur × temps de service de la plus grosse opération, plus une marge. En FIFO pur, le même scénario la dépasse. Le scénario rejoint `sweep`. |
| T02.06 : suite de tests commune des backends réussie sur ext4 et XFS | Le contrat commun des disques, avec `allocate`, sur le nouveau backend. En CI Linux, une étape le joue aussi sur un XFS monté en boucle (`mkfs.xfs`, `mount -o loop`). ext4 est le système des machines GitHub. |

Tests du `Scheduler`, sans thread :
- la file critique passe toujours la première ;
- parts de 50/25/15/10 à ±5 % sous saturation, sur 100 000 opérations de tailles variées ;
- le débit du fond ne dépasse pas sa part ;
- la profondeur n'est jamais dépassée ;
- la mesure de bande passante converge ;
- `proptest` : toute opération déposée sort une fois et une seule.

Tests réels : démarrage et arrêt des threads ; une lecture haute dépasse une file d'écritures basses ; file de dépôt pleine donnant `WouldBlock` ; `allocate`.

## 5. Risques

| Risque | Parade |
|---|---|
| Le journal (critique) affame les autres classes | Il est regroupé (écriture groupée, C) ; la latence des autres classes est surveillée par `ndaal_io_latency_seconds` |
| Le seau à jetons bride inutilement le fond quand le disque est libre | Voulu (isolation) ; la part se règle par `io.background_bandwidth_share`, à chaud plus tard |
| Le temps de service simulé ne ressemble pas au matériel | Les paramètres du `SimDevice` sont tirés de la graine dans des plages plausibles ; les bancs réels jugent le matériel (J07) |
| XFS indisponible sur les machines de CI | Échec explicite de l'étape, jamais un saut silencieux |
