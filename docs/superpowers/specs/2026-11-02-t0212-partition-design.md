# T02.12 — Abstraction `Partition` : spécification

**Tâche :** T02.12 ([CAHIER_DES_TACHES.md](../../../CAHIER_DES_TACHES.md#j02--noyau-de-stockage)).

**Exigences :** EF-TX-09, ENF-DUR-01, ENF-DUR-04, ADR-003, ADR-004, ADR-007.

**Sources :**
- [ARCHITECTURE §5](../../../ARCHITECTURE.md#5-espaces-de-noms-partitions-et-shards), §6 et §10.2 ;
- la spécification de J02-C2 ([2026-11-01-j02c2-wal-writer-design.md](2026-11-01-j02c2-wal-writer-design.md)).

**Statut :** conception validée par le responsable du projet le 2026-11-02, section par section.

## 1. Contexte et périmètre

Une partition appartient à un seul cœur. Elle a sa propre suite de LSN, indépendante du flux physique, et écrit dans le journal **partagé** de son shard (ADR-004). T02.12 fournit l'abstraction qui assemble journal et moteur, et sur laquelle s'appuieront les moteurs (J03 à J05), la reprise (T02.18) et Raft (T09.03).

**Hors du périmètre :**
- le rejeu au démarrage (T02.18), qui utilisera le même `Apply` ;
- les points de contrôle et le manifeste (T02.16, T02.17) ;
- le routage et l'assignation des partitions aux cœurs ;
- le mode `replicated` (J09).

## 2. Décisions

| # | Décision | Raison | Coût |
|---|---|---|---|
| 1 | **La partition applique les effets**, à travers un trait `Apply` implémenté par chaque moteur | Une seule fonction d'application pour l'écriture, la reprise (T02.18) et Raft (T09.03) : ce qui est écrit et ce qui est rejoué ne peuvent pas diverger | `Apply` n'a aujourd'hui qu'une implémentation de test ; exception à CLAUDE.md §8, justifiée par les trois moteurs de J03 à J05 |
| 2 | Les effets sont **calculés et validés par l'appelant avant** `submit` ; un échec de `apply` après l'écriture au journal fait passer la partition en `Failed` | Ne jamais diverger de son journal | Une partition en échec refuse toute écriture jusqu'au redémarrage |
| 3 | **`Busy` avant toute attribution de LSN** | Un refus ne change rien : l'appelant peut réessayer | — |
| 4 | Le **LSN durable passe par-dessus les entrées `None`** | Elles ne sont pas journalisées : rien n'est à rendre durable | Le LSN durable ne garantit pas la durabilité des entrées `None` (documenté) |
| 5 | **Toute attente a un délai** et répond `Timeout` | CLAUDE.md §4.4 | — |

## 3. Interface (crate `ndaal-storage`)

```rust
pub trait Apply {
    /// Applique les effets d'un enregistrement. Déterministe, et idempotent
    /// par version (ADR-007) : le même appel sert au rejeu.
    fn apply(&mut self, lsn: Lsn, kind: RecordType, payload: &[u8]) -> Result<(), ApplyError>;
}
```

`Partition<D: Disk, C: Clock, E: Apply>`, sur le cœur du shard (pas `Send`) :

| Élément | Rôle |
|---|---|
| `new(id, writer: WalWriter<D, C>, engine: E, clock: C, registry)` | Partition neuve : aucun LSN attribué |
| `resume(applied: Lsn)` | Repart d'un LSN connu (après un point de contrôle ou une reprise) : le LSN suivant est `applied + 1` |
| `submit(kind, payload, mode: Durability) -> Result<Lsn, PartitionError>` | Ordre de ARCHITECTURE §10.2 : refus `Busy` éventuel, LSN suivant, ajout au journal (sauf `None`), application, puis réponse selon le mode (`None` et `Async` tout de suite, `Sync` après la durabilité) |
| `applied_lsn()`, `durable_lsn()`, `last_lsn()` | Les trois repères |
| `wait_applied(lsn, timeout)` | Se termine quand `lsn` est appliqué, même s'il n'est pas encore attribué ; `Timeout` sinon |
| `wait_durable(lsn, timeout)` | Se termine quand `lsn` est appliqué puis durable ; `Timeout` sinon |
| `engine()` | Accès en lecture au moteur (lectures des frontaux) |

`PartitionError` (`thiserror`) : `Busy`, `Failed`, `Timeout`, `Wal(WalError)` (cause d'origine conservée).

## 4. Suivi des repères

- **LSN appliqué** : avance de façon synchrone dans `submit` ; un `Notify` réveille les `wait_applied`.
- **LSN durable** : la partition garde, dans l'ordre, la position dans le journal de chaque LSN pas encore durable. Le LSN durable est le plus grand LSN dont toutes les entrées journalisées qui le précèdent ont une position couverte par `writer.durable_position()`. Les entrées `None` ne bloquent pas son avance.
- **`wait_durable`** attend d'abord l'application, puis `writer.durable(position)`, le tout sous le même délai.

## 5. Métriques (EXPLOITATION §8)

| Nom | Type |
|---|---|
| `ndaal_wal_durable_lsn{partition}` | jauge |
| `ndaal_partition_applied_lsn{partition}` | jauge |
| `ndaal_partition_failed_total{partition}` | compteur |

## 6. Tests et critère de fin

Critère de T02.12 : « API documentée ; tests d'application et d'attente ». Les tests tournent sur le simulateur (`ndaal-sim/tests/partition.rs`), avec un moteur de test qui enregistre chaque application.

| Critère | Preuve |
|---|---|
| Application dans l'ordre | Les LSN appliqués valent 1, 2, 3… ; un `Busy` ne consomme pas de LSN |
| Modes | `None` est appliqué mais absent du journal ; `Async` répond avant la durabilité ; `Sync` après |
| Attentes | `wait_applied` sur un LSN futur se réveille à son application ; `Timeout` à l'expiration ; `wait_durable` se termine après le `fdatasync` qui couvre l'entrée |
| Deux partitions, un journal | Suites de LSN indépendantes, LSN durables justes pour chacune |
| Échec d'application | `Failed`, écritures suivantes refusées, échec compté |
| Propriété | Soumissions et attentes aléatoires : LSN durable ≤ LSN appliqué ; tout `Sync` acquitté est durable |
| `resume` | Une partition reprise à un LSN donné attribue le suivant |

Chaque élément public porte sa documentation `///` (ENF-DOC-02). Tout défaut corrigé pendant la tâche ajoute une ligne au [registre des défauts](../../recherche/registre-des-defauts.md).

## 7. Risques

| Risque | Parade |
|---|---|
| Un moteur dont `apply` n'est pas idempotent fausse le rejeu (T02.18) | Exigence écrite dans la documentation du trait ; tests de rejeu en T02.18 |
| Beaucoup de partitions par nœud : cardinalité des métriques | Étiquette `partition` bornée par le nombre de partitions locales ; à revoir avec la cardinalité bornée d'EXPLOITATION §8 si besoin |
| Une partition en échec reste refusée jusqu'au redémarrage | Voulu : ne jamais diverger du journal ; la reprise la reconstruit |
