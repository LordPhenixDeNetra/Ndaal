# J02-A — Runtime thread-per-core : spécification

**Tâches :** T02.01, T02.02, T02.03 ([CAHIER_DES_TACHES.md](../../../CAHIER_DES_TACHES.md#j02--noyau-de-stockage)).

**Exigences :** ADR-003, ADR-017, ENF-PERF-01, ENF-ISO-01 et 02, ENF-RES-02, P8.

**Sources :**
- [ARCHITECTURE §4.1 et §4.2](../../../ARCHITECTURE.md#4-modèle-dexécution) ;
- [EXPLOITATION §3](../../../EXPLOITATION.md#3-référence-de-configuration) (`[runtime.cores]`) ;
- [DECISIONS](../../../DECISIONS.md) ADR-003, ADR-016, ADR-017.

**Statut :** conception validée par le responsable du projet le 2026-10-15, section par section.

## 1. Contexte et périmètre

J02 (noyau de stockage) est découpé en six sous-projets, chacun avec sa spécification et son plan :

| # | Sous-projet | Tâches |
|---|---|---|
| A | Runtime | T02.01 à T02.03 |
| B | Ordonnancement | T02.04 à T02.06 |
| C | Journal | T02.07 à T02.11, T02.22 |
| D | Segments | T02.13 à T02.16 |
| E | Partition et reprise | T02.12, T02.17 à T02.19, T02.26 |
| F | Transverse | T02.20, T02.21, T02.23 à T02.25 |

Ce document couvre **A**.

**Dans le périmètre :**
- un thread par cœur, chacun avec un runtime `tokio` `current_thread`, épinglé quand le système le permet ;
- l'attribution des cœurs aux classes de charge ;
- la messagerie bornée entre cœurs, en version réelle et en version simulée ;
- la configuration `[runtime.cores]` et `runtime.idle_spin` ;
- les métriques du runtime.

**Hors du périmètre :**
- la priorité stricte avec budget de 10 % sur un cœur partagé (T02.04, sous-projet B) : d'ici là, un cœur partagé sert ses tâches dans l'ordre d'arrivée ;
- l'ordonnanceur d'E/S ;
- l'admission par classe (ARCHITECTURE §4.4) ;
- l'acceptation des connexions avec `SO_REUSEPORT`, qui viendra avec le frontal RESP (J03).

## 2. Décisions

| # | Décision | Raison |
|---|---|---|
| 1 | **Deux implémentations** de la messagerie, réelle et simulée, derrière un même trait (comme `ndaal-io`, CLAUDE.md §8) | Les défauts entre cœurs (file pleine, ordre des réponses, interblocage) doivent être reproductibles depuis une graine (ADR-017) |
| 2 | File SPSC réelle : crate **`rtrb`** 0.4 (MIT OR Apache-2.0, Rust pur, sans attente, bornée), enveloppée | Aucun `unsafe` à écrire ni à prouver ; dépendance non structurante, vérifiée par `cargo deny` |
| 3 | Épinglage : crate **`core_affinity`** 0.8 (MIT/Apache-2.0) | `sched_setaffinity` sous Linux, `SetThreadAffinityMask` sous Windows ; elle liste les cœurs autorisés (cgroup, `taskset`) ; aucun `unsafe` de notre côté |
| 4 | Cœur inactif : **attente active puis sommeil**. Le cœur surveille ses files pendant `runtime.idle_spin` (50 µs par défaut) en laissant passer ses autres tâches, puis s'endort ; l'émetteur ne fait l'appel système de réveil que si le destinataire dort | < 1 µs sous charge ; aucune consommation de CPU au repos au-delà du délai |
| 5 | **Messages typés** : le runtime est générique sur le type de message `M` fourni par la couche au-dessus | Aucune allocation par message sur le chemin court (CLAUDE.md §4.5) ; messages lisibles dans les traces de simulation |

Ni `rtrb` ni `core_affinity` ne sont des dépendances structurantes au sens de CLAUDE.md §4.11 : elles n'appellent pas d'ADR.

## 3. Composants

La crate `ndaal-runtime` reçoit quatre modules, chacun sous les 400 lignes visées.

### 3.1 `layout` : attribution des cœurs

```rust
pub enum Class { Shard, Vector, Transfer, Background, Io }

pub struct Layout { /* une entrée par cœur : classe principale, classe partagée éventuelle */ }

pub fn assign(available: &[CoreId], config: &CoresConfig) -> Result<Layout, LayoutError>
```

C'est une fonction pure, sans thread ni effet de bord. Ses règles :

1. **Lignes de la table.** Les lignes 2, 4, 8, 16 et 64 cœurs de la table d'ARCHITECTURE §4.1 sont reproduites exactement.
2. **Entre deux lignes**, chaque classe est interpolée linéairement entre ses deux lignes voisines. Les parts sont arrondies par la méthode du plus grand reste : la somme fait exactement le nombre de cœurs.
3. **Au-delà de 64 cœurs**, les proportions de la ligne 64 sont conservées, avec le même arrondi.
4. **Avec 1 cœur**, toutes les classes partagent ce cœur, la classe `shard` en premier.
5. **Classe sans cœur dédié.** Elle est partagée avec la classe que la table désigne (par exemple `background` avec `transfer` sur 8 cœurs). Entre deux lignes, c'est la désignation de la ligne inférieure qui s'applique.
6. **Valeurs imposées.** `runtime.cores.<classe> = N` impose un nombre ; les classes laissées à `auto` se répartissent le reste selon les règles 1 à 5. Les cas suivants sont des `LayoutError` qui nomment la clé, et deviennent des erreurs de configuration (code 2) :
   - une somme supérieure aux cœurs disponibles ;
   - `shard = 0` ;
   - une valeur négative ou non entière.
7. **Cœurs disponibles.** `runtime.cores.total = "auto"` prend les cœurs autorisés pour le processus ; un nombre explicite prend les N premiers d'entre eux, et un nombre supérieur aux cœurs autorisés est une erreur.

### 3.2 `mailbox` : messagerie entre cœurs

Le trait est générique sur `M` (message) et `R` (réponse) :

| Opération | Contrat |
|---|---|
| `try_send(dest, M) -> Result<(), SendError<M>>` | N'attend jamais. `SendError::Full(M)` rend le message ; l'appelant répond « occupé » (P8). `SendError::ShuttingDown(M)` après l'arrêt. |
| `request(dest, M) -> Result<Pending<R>, SendError<M>>` | Réserve d'abord une place dans la file de retour de `dest` vers l'émetteur. Sans place libre, c'est `Full`, sans rien envoyer. `Pending<R>` est un futur résolu par la réponse. |
| `recv() -> Incoming<M, R>` | Côté destinataire : sert les files entrantes, dans l'ordre FIFO pour chaque émetteur. Une requête reçue porte un `Replier<R>` à usage unique. |
| `Replier::reply(R)` | Ne peut pas échouer faute de place, puisque la place a été réservée par `request`. |

Une file par paire ordonnée de cœurs (émetteur, destinataire) et par direction, de capacité fixe, réglable à la construction. Garanties :
- **pas de perte** : un message accepté est livré ;
- **pas de doublon** ;
- **ordre FIFO par paire** ;
- **pas d'interblocage requête/réponse**, grâce à la réservation ;
- **nombre de requêtes en cours borné** vers un cœur donné.

Un `Replier` abandonné sans réponse (bogue) libère sa place réservée et résout le `Pending` en `Dropped`. Il ne bloque jamais l'émetteur.

### 3.3 `real` : threads, épinglage, sommeil

- `Runtime::start(layout, config, factory)` démarre un thread par cœur du `Layout`. Chaque thread construit un runtime `tokio` `current_thread`, épingle le thread si `runtime.cores.pin` vaut vrai, puis exécute la tâche que la fabrique produit pour son cœur.
- `start` attend que tous les cœurs soient prêts, **5 s au plus**. Les erreurs remontent, sans panique, et les threads déjà démarrés sont arrêtés :
  - échec de création d'un thread ;
  - échec d'épinglage sous Linux ou Windows ;
  - délai dépassé.
- **Épinglage impossible** (macOS) : un avertissement `runtime pinning unsupported` est journalisé une fois, et le runtime continue sans épinglage.
- **Réveil.** Chaque cœur a un drapeau « endormi » et un signal de réveil (`tokio::sync::Notify`, sûr entre threads). L'émetteur ne fait l'appel de réveil que si le drapeau est levé. Le drapeau est relu après la publication du message, pour ne jamais perdre un réveil.
- **Arrêt.** `shutdown()` marque chaque cœur « en arrêt » : les nouveaux envois rendent `ShuttingDown`, les tâches en cours se terminent, et les threads sont rejoints, **10 s au plus**.
  - Un cœur qui dépasse le délai est journalisé (`runtime core did not stop`) et compté.
  - `shutdown()` renvoie alors une erreur nommant les cœurs en cause.
- **Panique** sur un cœur : arrêt de tout le processus (`panic = "abort"`, ADR-030).

### 3.4 `sim` : implémentation simulée

- Chaque cœur est un ensemble de tâches dans l'exécuteur unique de `ndaal-sim`. Les files ont la **même capacité** qu'en réel, donc `Full` se produit aussi en simulation.
- Livraison :
  - **FIFO par paire** ;
  - l'entrelacement entre émetteurs différents et un délai de livraison sont tirés d'un générateur nommé de la simulation (`sim.rng("runtime")`), puis tracés dans l'empreinte ;
  - pas d'épinglage ni d'attente active : seul le temps virtuel avance.
- `ndaal-runtime` ne dépend pas de `ndaal-sim`, puisque les dépendances vont vers le bas. L'implémentation simulée s'appuie sur l'horloge et l'aléa simulés de `ndaal-io`, et l'exécuteur de `ndaal-sim` la fait tourner.

### 3.5 Configuration

Nouveaux paramètres dans `ndaal-common`, visibles dans `ndaal config reference` et documentés dans EXPLOITATION §3 :

| Clé | Type | Défaut |
|---|---|---|
| `runtime.cores.total` | `auto` ou entier ≥ 1 | `auto` |
| `runtime.cores.shard`, `.vector`, `.transfer`, `.background`, `.io` | `auto` ou entier ≥ 0 (`shard` ≥ 1) | `auto` |
| `runtime.cores.pin` | booléen | `true` |
| `runtime.idle_spin` | durée (`0` = s'endormir aussitôt) | `50us` |

Le type « durée » est nouveau dans `ndaal-common` (`Kind::Duration` : `us`, `ms`, `s`).

### 3.6 Métriques

| Nom | Type | Étiquettes |
|---|---|---|
| `ndaal_runtime_queue_full_total` | compteur | `to` (cœur destinataire : au plus autant de séries que de cœurs, pas leur carré) |
| `ndaal_runtime_core_parks_total` | compteur | `core`, `queue` (`messages` ou `replies` : la tâche de réception et la pompe des réponses s'endorment séparément) |
| `ndaal_runtime_replies_dropped_total` | compteur | — |
| `ndaal_runtime_cores` | jauge | `class` |

## 4. Tests et critères de fin

| Critère de fin (cahier) | Preuve |
|---|---|
| T02.01 : N threads épinglés démarrent et s'arrêtent proprement ; fonctionne sans épinglage sur macOS et Windows | Test de démarrage et d'arrêt de 4 cœurs sur les trois systèmes en CI. L'épinglage est vérifié par `core_affinity` sous Linux ; sous macOS, on vérifie l'avertissement et le fonctionnement |
| T02.02 : table respectée de 2 à 64 cœurs | Tests unitaires de `layout` : les cinq lignes exactes, et de 1 à 256 cœurs (somme, au moins un `shard`, toute classe non dédiée partagée, valeurs imposées respectées, configurations impossibles refusées) |
| T02.03 : aller-retour < 1 µs entre deux cœurs ; comportement correct quand la file est pleine | Micro-banc `criterion` `runtime/round_trip` (deux cœurs actifs, requête puis réponse), ajouté aux bancs nocturnes. L'objectif vaut sur la machine de référence (D-05) ; il est noté, sans garantie, sur les machines partagées de GitHub. Le cas de la file pleine est couvert par le contrat commun ci-dessous. |

Autres tests :
- **Contrat commun de la messagerie**, exécuté sur les deux implémentations, sur le modèle du contrat des disques de `ndaal-io` :
  - FIFO par paire ;
  - `Full` rend le message ;
  - réponse toujours livrée grâce à la réservation ;
  - requête refusée sans place de retour ;
  - `Replier` abandonné donnant `Dropped` et libérant la place ;
  - `ShuttingDown` après l'arrêt.
- **Réveil**, en réel : un message envoyé pendant la mise en sommeil est toujours reçu, sur 100 000 essais.
- **Scénario de simulation `cross_core`**, ajouté à `ndaal-sim sweep` et donc à la simulation nocturne :
  - N cœurs s'envoient des requêtes en rafale sur des files de petite capacité ;
  - aucune requête perdue ni dupliquée, chaque réponse arrive à son demandeur, et aucun interblocage (le scénario se termine) ;
  - reproductible depuis sa graine.
- **Configuration** : les nouveaux paramètres sont acceptés, et leurs valeurs invalides refusées avec le nom de la clé.

## 5. Risques

| Risque | Conséquence | Parade |
|---|---|---|
| Interblocage requête/réponse entre deux cœurs | Shards figés | Réservation de la place de retour (§3.2), et scénario `cross_core` sur 10 000 graines chaque nuit |
| Réveil perdu (course entre la mise en sommeil et l'envoi) | Message reçu en retard, ou jamais | Drapeau relu après publication (§3.3), et test de 100 000 essais |
| Bruit des machines partagées sur le micro-banc | Faux dépassements de l'objectif de 1 µs | Objectif jugé sur la machine de référence (D-05, P-50) |
| `rtrb` ou `core_affinity` abandonnées | Mise à jour bloquée | Elles sont enveloppées derrière nos types : un remplacement ne touche que `real` |
