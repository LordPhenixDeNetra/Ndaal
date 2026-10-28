# J02-B1 — Ordonnanceur d'E/S et pool de threads : plan d'implémentation

> **Pour les agents :** sous-compétence requise : superpowers:executing-plans, tâche par tâche.
>
> **Pas d'étape de commit** : le responsable commite sur une branche `j02b1-io-scheduler` et ouvre une demande de fusion. Le compte rendu donne les commandes git **et le contenu du formulaire de demande de fusion**.

**Objectif :** un ordonnanceur d'E/S par périphérique, qui sert le même code au disque réel et au disque simulé :
- classe critique prioritaire ;
- parts 50/25/15/10 au tourniquet à déficit ;
- seau à jetons du fond, réglé sur la bande passante mesurée ;
- profondeur bornée.

S'y ajoutent un pool de threads d'E/S dédié pour le disque réel (T02.06), un modèle de périphérique pour la simulation, `File::allocate`, la configuration `[io]` et les métriques `ndaal_io_*`.

**Architecture :**
- `ndaal_io::sched::Scheduler` est une machine à états pure : le temps est passé en paramètre, sans thread ni horloge.
- **Côté réel**, `real::IoPool` tient un `Scheduler` par périphérique dans un thread répartiteur, et confie le travail à `io.queue_depth` threads. Le disque réel passe d'un `spawn_blocking` à ce pool, sans réécrire ses opérations : sa fonction `blocking(work)` devient un appel à `IoSubmitter::run(class, cost, work)`.
- **Côté simulé**, `sim::SimDevice` tient le même `Scheduler`, modélise un temps de service et entoure `SimDisk`.

**Pile :** `rtrb` (déjà présente), `libc` (fonction `fallocate` sous Linux), `proptest`.

**Spécification :** [docs/superpowers/specs/2026-10-26-j02b1-io-scheduler-design.md](../specs/2026-10-26-j02b1-io-scheduler-design.md), validée le 2026-10-26. En cas de doute, elle fait foi.

## Contraintes globales

- Valeurs fixées par la spécification :
  - poids 50/25/15/10 (`High`, `Medium`, `Low`, `Background`) ;
  - coût d'une opération sans taille : 4 096 octets ;
  - capacité du seau = 100 ms du débit du fond ;
  - fenêtre de mesure de 100 ms, valeur de départ 100 Mio/s ;
  - `io.queue_depth` de 1 à 1024, 32 par défaut ;
  - `io.background_bandwidth_share` de 0 à 1, 0,30 par défaut ;
  - arrêt du pool en 10 s au plus.
- **Effets de bord (ADR-017).** Le `Scheduler` ne lit aucune horloge. Le pool et le `SimDevice` passent par `Clock` (`SystemClock` ou `SimClock`).
- **`unsafe`.** Permis dans `ndaal-io` pour le seul appel `libc::fallocate`, avec `// SAFETY:`.
- **Pas de verrou sur le chemin des cœurs** (ADR-003). Les dépôts passent par des files `rtrb`, une par déposant. Une file pleine donne `io::ErrorKind::WouldBlock` ; un pool arrêté donne `io::ErrorKind::Interrupted`.
- **Taille des fichiers.** 400 lignes visées : `sched/` est découpé en `mod.rs` (types), `scheduler.rs` (choix) et `bandwidth.rs` (mesure, seau).
- **Code.** ASCII, aucun emoji, test d'abord, mutation quand un test passe du premier coup.

## Points à surveiller en revue

1. **Le fond seul, sur un disque libre**, reste bridé à sa part : il ne prend jamais tout le débit. Testé à la tâche 3.
2. **Une classe vide** ne garde pas de crédit d'avance (déficit remis à zéro quand sa file est vide), sinon elle passerait en rafale à son retour. Testé à la tâche 3.
3. **Une opération plus grosse que le seau entier** (une écriture de fond de 64 Mio) finit par passer : le seau peut descendre sous zéro, au lieu de bloquer pour toujours. Testé à la tâche 3.
4. **Arrêt du pool avec des opérations en attente** : elles échouent en `Interrupted`, celles en cours se terminent, et le pool s'arrête en moins de 10 s. Testé à la tâche 5.
5. **Le temps virtuel n'avance jamais seul** en simulation : un `SimDevice` sans opération ne réveille rien, et le scénario se termine. Testé à la tâche 4.

---

### Tâche 1 : configuration `[io]`

**Fichiers :** `crates/ndaal-common/src/config/{params.rs, runtime.rs (ou io.rs), mod.rs}`, `crates/ndaal-common/tests/config_io.rs`, `crates/ndaal-cli/src/error.rs`, `locales/{en,fr}/cli.ftl`, `EXPLOITATION.md` §3.

**Interfaces (produit) :**
- `Kind::Ratio`, de `0` à `1`, au plus 6 décimales (`0.3`, `0.30`, `1`).
- `Kind::Range(min, max)` pour un entier borné.
- `pub struct Ratio(u32)`, en parts par million (exact, donc `Eq`), avec `pub fn as_f64(self) -> f64`.
- `pub enum IoBackend { Auto, Threads }`.
- `pub struct IoConfig { pub backend: IoBackend, pub queue_depth: u32, pub background_share: Ratio }`, accessible par `Config.io`.
- Clés Fluent : `cli-expected-ratio`, `cli-expected-range` (`{ $min }`, `{ $max }`).

- [ ] **Tests d'abord :**
  - `io_defaults` : `Auto`, 32 et 300 000 ppm ;
  - `ratios` : `0`, `0.3`, `1` et `0.000001` sont acceptés ; `1.1`, `-0.1`, `.5`, `0.1234567` et `x` sont refusés ;
  - `queue_depth_range` : `1` et `1024` sont acceptés, `0` et `1025` refusés ;
  - `io_backend_choice` : `uring` est refusé (J07) ;
  - `render` puis `load` redonnent la même configuration ;
  - CLI : un ratio invalide est expliqué en français.

---

### Tâche 2 : `File::allocate`

**Fichiers :** `crates/ndaal-io/src/{disk.rs, real/disk.rs, sim/disk.rs, sim/ops.rs}`, `crates/ndaal-io/tests/common/mod.rs`, `crates/ndaal-io/Cargo.toml` (`libc` sous Linux).

**Interface :** `async fn allocate(&self, len: u64) -> io::Result<()>` dans le trait `File`. Elle réserve l'espace jusqu'à `len` sans changer le contenu, et la taille du fichier devient `max(taille, len)`.
- Linux : `libc::fallocate(fd, 0, 0, len)`, puis un repli sur `set_len` si le système de fichiers ne la prend pas en charge (`EOPNOTSUPP`).
- Ailleurs : `set_len(len)` si le fichier est plus court.
- Disque simulé : une écriture de zéros en attente, perdue en cas de panne avant `sync_data`, comme `set_len`.

- [ ] **Test d'abord**, dans le contrat commun `disk_contract` :
  - `allocate(8192)` sur un fichier de 10 octets donne une taille de 8 192, les 10 premiers octets intacts et des zéros ensuite ;
  - `allocate(4)` ne tronque pas.

  Le test tourne sur les disques réel et simulé.

---

### Tâche 3 : `sched::Scheduler`

**Fichiers :** créer `crates/ndaal-io/src/sched/{mod.rs, scheduler.rs, bandwidth.rs}` et `crates/ndaal-io/tests/sched.rs` ; modifier `crates/ndaal-io/src/lib.rs`.

**Interfaces (produit) :**
- `pub enum IoClass { Critical, High, Medium, Low, Background }`.
- `pub struct OpId(pub u64)`.
- `pub struct Op { pub id: OpId, pub class: IoClass, pub cost: u64 }` : le coût est en octets, 4 096 au minimum.
- `pub struct SchedulerConfig { pub depth: usize, pub background_share: f64 }`.
- `pub struct Scheduler`, avec :
  - `pub fn new(config, now: Duration) -> Self` ;
  - `pub fn submit(&mut self, op: Op)` ;
  - `pub fn next(&mut self, now: Duration) -> Option<Op>` : rend `None` si la profondeur est atteinte, si tout est vide, ou si seul le fond attend sans jetons ;
  - `pub fn complete(&mut self, op: &Op, bytes: u64, now: Duration)` ;
  - `pub fn next_wakeup(&self, now: Duration) -> Option<Duration>` : quand le fond aura des jetons ;
  - `pub fn bandwidth(&self) -> u64`, en octets par seconde ;
  - `pub fn inflight(&self) -> usize` ;
  - `pub fn drain(&mut self) -> Vec<Op>`, qui vide les files à l'arrêt.
- Tourniquet à déficit :
  - quantum = poids × 4 096 octets par tour ;
  - le déficit d'une classe dont la file est vide est remis à 0 ;
  - le seau du fond peut devenir négatif : une grosse opération passe, et le fond attend ensuite de rembourser.

- [ ] **Tests d'abord :**
  - `critical_first` ;
  - `weighted_shares` : 4 classes saturées, 100 000 opérations de 4 Kio à 1 Mio tirées d'un `Xoshiro256PlusPlus` à graine fixe ; parts d'octets servis à ±5 % de 50/25/15/10 (fond limité au besoin par une bande passante assez haute) ;
  - `background_bucket_caps_rate` : fond seul, débit servi au plus égal à la part de la bande passante mesurée (+5 %), sur 10 s simulées ;
  - `oversized_background_op_passes` ;
  - `empty_class_keeps_no_credit` ;
  - `depth_is_never_exceeded` ;
  - `bandwidth_converges` : à débit réel fixe de 200 Mio/s, la mesure arrive à ±5 % en 1 s ;
  - `proptest` : toute suite de dépôts, avec des `complete` intercalés, rend chaque opération exactement une fois.

---

### Tâche 4 : `sim::SimDevice` et le scénario `io_priority` (T02.05)

**Fichiers :** créer `crates/ndaal-io/src/sim/device.rs` et `crates/ndaal-sim/src/io_priority.rs` ; modifier `crates/ndaal-io/src/sim/mod.rs`, `crates/ndaal-sim/src/{lib.rs, main.rs}` et `.github/workflows/nightly.yml` (rien à changer si `sweep` prend le scénario).

**Interfaces :**
- `pub struct DeviceModel { pub latency: Duration, pub bytes_per_second: u64 }`.
- `pub enum Policy { Priority, Fifo }` : `Fifo` sert de témoin, pour montrer que le test discrimine.
- `pub struct SimDevice` (`Rc`), avec :
  - `pub fn new(clock: SimClock, rng: Xoshiro256PlusPlus, model, config: SchedulerConfig, policy) -> Self` ;
  - `pub async fn run<T>(&self, class: IoClass, cost: u64, work: impl Future<Output = T>) -> T` : dépôt, attente du tour, temps de service (latence plus coût divisé par le débit, ±10 % tirés de la graine), exécution de `work`, puis `complete`.
- `pub struct DeviceDisk` entoure un `SimDisk` et un `SimDevice` :
  - `open_class(path, options, class) -> DeviceFile` ;
  - les opérations de `Disk` (renommer, supprimer, lister, synchroniser un dossier) en `Critical` ;
  - `DeviceFile` implémente `File`.
- `pub fn io_priority(seed: u64, policy: Policy) -> IoPriorityReport { outcome, high_p99: Duration, high_reads: u32, fingerprint }` :
  - 200 Mio/s, 100 µs, profondeur 4 ;
  - rafale continue d'écritures `Low` de 1 Mio pendant 1 s ;
  - une lecture `High` de 4 Kio par ms.
- Borne : `HIGH_P99_BOUND = depth × service(1 Mio) + 2 ms`.
- `pub fn io_priority_result(seed) -> SeedResult` : réussite si `Priority` donne un p99 sous la borne.

- [ ] **Tests d'abord :**
  - `high_reads_stay_under_the_bound` : 20 graines ;
  - `fifo_breaks_the_bound` : en `Fifo`, la même graine dépasse la borne ;
  - `io_priority_is_reproducible` ;
  - `idle_device_lets_the_sim_finish` ;
  - le contrat commun `disk_contract` passe sur un `DeviceDisk`.
- [ ] `ndaal-sim sweep` joue `io_priority` en plus des deux autres scénarios ; la durée de 1 000 graines est consignée dans le registre.

---

### Tâche 5 : `real::IoPool` et le disque réel sur le pool (T02.06)

**Fichiers :** créer `crates/ndaal-io/src/real/pool.rs` et `crates/ndaal-io/tests/real_pool.rs` ; modifier `crates/ndaal-io/src/real/{disk.rs, mod.rs}`.

**Interfaces (produit) :**
- `pub struct IoPool`, avec :
  - `pub fn start(device: &str, config: SchedulerConfig, cpus: &[usize], registry: &Registry) -> io::Result<IoPool>` ;
  - `pub fn submitter(&self) -> IoSubmitter`, qui est `Send` et donne une file `rtrb` par déposant, enregistrée auprès du répartiteur ;
  - `pub fn shutdown(self) -> Result<(), PoolShutdownError>`.
- `IoSubmitter::run<T: Send + 'static>(&self, class: IoClass, cost: u64, work: impl FnOnce() -> io::Result<T> + Send + 'static) -> impl Future<Output = io::Result<T>>`.
- **Le répartiteur** est un thread : il lit les files de dépôt, appelle `Scheduler::next`, et confie le travail à `depth` threads d'E/S par un canal borné à `depth`.
  - Les threads d'E/S sont épinglés sur `cpus` (les cœurs `io` du `Layout`) quand c'est possible.
  - Le répartiteur s'endort sans travail, et un dépôt le réveille avec le même schéma de signal qu'en J02-A (`Signal` à déplacer dans un module commun de `ndaal-io`).
  - Il attend jusqu'à `next_wakeup` quand seul le fond attend des jetons.
- `RealDisk` devient `RealDisk { exec: Exec }`, avec `enum Exec { Blocking, Pool(IoSubmitter, IoClass) }` :
  - `RealDisk` (`Default`) garde `spawn_blocking`, pour la CLI et les tests existants ;
  - `RealDisk::on_pool(submitter)` et `open_class(path, options, class)` passent par le pool.

  Le coût vaut la taille du tampon pour les lectures et écritures, 4 096 sinon.
- Métriques `ndaal_io_*` de la spécification §3.7 (étiquette `device` = nom passé à `start`).

- [ ] **Tests d'abord :**
  - `disk_contract_on_the_pool` : le contrat commun sur `RealDisk::on_pool` ;
  - `high_overtakes_queued_low` : profondeur 1, une écriture `Low` en cours, 20 `Low` en attente, puis une lecture `High` : elle passe avant les 20 ;
  - `full_submission_queue_is_would_block` ;
  - `shutdown_fails_waiting_ops` : `Interrupted`, arrêt en moins de 10 s ;
  - `metrics_are_exposed`.

---

### Tâche 6 : CI XFS, documentation, vérifications

- [ ] `.github/workflows/ci.yml`, tâche `test` sous `ubuntu-24.04`, nouvelle étape « Disk contract on XFS » :
  - créer un fichier de 1 Gio, `mkfs.xfs`, le monter en boucle (`sudo mount -o loop`) et en donner la propriété à l'utilisateur ;
  - lancer `NDAAL_TEST_DIR=<point de montage> cargo nextest run -p ndaal-io --test real_disk --test real_pool` ;
  - les deux fichiers de test lisent `NDAAL_TEST_DIR` (sinon un dossier temporaire) ;
  - si `mkfs.xfs` manque, l'étape échoue, sans saut silencieux.
- [ ] Documents :
  - ARCHITECTURE §4.3 : implémentation réelle (tourniquet à déficit, seau sous zéro, pool) ;
  - EXPLOITATION §3 : `[io]` ;
  - STRATEGIE §13.4 : `sweep` avec trois scénarios ;
  - POINTS_REPORTES : réglage automatique selon la latence (J07) et les points restants.
- [ ] Chaîne complète de CLAUDE.md §11, dont `check-linux.sh` et `check-workflows.sh` ; revue finale par un relecteur neuf ; compte rendu, commandes git et formulaire de demande de fusion.
- [ ] Cochage de T02.05 et T02.06 après la CI verte, XFS compris.
