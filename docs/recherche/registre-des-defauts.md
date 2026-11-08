# Registre des défauts trouvés

Tâche [T15.01](../../CAHIER_DES_TACHES.md#j15--publications-scientifiques). Ce registre consigne chaque défaut **trouvé et corrigé**, avec la technique qui l'a révélé. Il sert à mesurer ce que rapporte chaque technique de vérification (tests de propriétés, simulation déterministe, fuzzing, revues, mutations), et nourrit les sections de validation des articles (J15).

## Règles

- **Une ligne par défaut corrigé**, ajoutée dans la même modification que la correction. Un point non corrigé va dans [POINTS_REPORTES.md](../../POINTS_REPORTES.md), pas ici.
- **Technique** : ce qui a fait apparaître le défaut, pas ce qui prouve la correction. Valeurs : revue de conception, revue finale (relecteur neuf), revue de l'auteur, test (échec), test intermittent, test par mutation, simulation (graine), fuzzing, lint, `cargo deny`, exécution (banc ou CI).
- **Gravité** :
  - **critique** : une écriture acquittée peut être perdue ou une donnée corrompue servie (ENF-DUR) ;
  - **important** : panique, blocage, refus de service, fausse alerte de corruption, garantie fausse, sécurité, ou un test qui ne peut pas détecter ce qu'il vérifie ;
  - **mineur** : le reste (outillage, confort, test mal dimensionné).
- **Test ajouté** : le test qui échouait avant la correction (CLAUDE.md §5), ou « aucun » avec la raison.

## Limites de ce registre

- Ouvert le 2026-11-01. Les lignes de **J01** sont reconstituées à partir de l'historique git, du [registre des modifications du plan](../../CAHIER_DES_TACHES.md#registre-des-modifications-du-plan), du [journal des jalons](../../CAHIER_DES_TACHES.md#journal-des-jalons) et des points reportés : la plupart des corrections de J01 ont été intégrées aux commits de fonctionnalité sans trace séparée, donc **J01 est incomplet**.
- Les lignes de **J02** viennent des registres de travail des sous-projets J02-A à J02-C2 et de leurs revues finales.

## Synthèse

| Technique | Défauts | Dont importants |
|---|---|---|
| Revue finale (relecteur neuf) | 17 | 15 |
| Revue de conception | 1 | 1 |
| Revue (J01, plan) | 2 | 2 |
| Revue de l'auteur | 1 | 0 |
| Test (échec) | 3 | 1 |
| Test intermittent | 3 | 0 |
| Test par mutation | 2 | 2 |
| Simulation (graine) | 2 | 0 |
| Lint | 1 | 0 |
| `cargo deny` | 1 | 1 |
| Exécution (banc ou CI) | 4 | 1 |
| **Total** | **37** | **23** |

Aucun défaut **critique** n'a été trouvé à ce jour. Aucun défaut n'a encore été révélé par le **fuzzing** ni par un **test de propriétés** : ils ont servi à prouver les corrections. Les deux défauts trouvés par **mutation** portaient sur le modèle de pannes de la simulation elle-même, qui ne pouvait pas produire le cas qu'il devait vérifier.

## Registre

| N° | Date | Tâche | Composant | Défaut | Technique | Gravité | Correction | Test ajouté |
|---|---|---|---|---|---|---|---|---|
| DEF-01 | 2026-10-13 | T01.05, T01.07 | `ndaal-common`, `ndaal-io` | Le LSN n'avait qu'un `next()` qui panique au débordement, atteignable depuis des données lues ; le contrat des disques ne couvrait ni le renommage sur une cible existante, ni `NotFound`, ni la lecture d'un fichier ouvert puis supprimé (P-18, P-20) | Revue (J01) | Important | `Lsn::checked_next` ; contrat commun complété | Contrat commun des disques, réel et simulé |
| DEF-02 | 2026-10-14 | T01.20 | Plan | T01.20 cochée sur son critère de fin, alors que la simulation nocturne exigée pour clore un jalon n'existait pas | Revue (plan) | Important | Tâche T01.26 ajoutée (`nightly.yml`) | Aucun (plan) |
| DEF-03 | 2026-10-14 | T01.18 | CI des bancs | Après la création de la branche `bench-data`, `git switch -` échouait : le dépôt cloné par Actions n'a pas de branche précédente | Exécution (CI) | Mineur | Retour à `GITHUB_SHA` | Aucun (workflow) |
| DEF-04 | 2026-10-14 | T01.18 | CI de la documentation | L'extraction de `bench-data` encore vide échouait (`pathspec '.'`) | Exécution (CI) | Mineur | `git archive` | Aucun (workflow) |
| DEF-05 | 2026-10-25 | T01.18 | Banc nocturne | En échec du 14 au 24/10 (sauf le 21) : référence `criterion` périmée dans le cache | Exécution (CI nocturne) | Mineur | Corrigé à la clôture de J01 | Aucun (workflow) |
| DEF-06 | 2026-10-25 | T02.03 | `ndaal-runtime` (tests) | Un test utilisait `recv_timeout`, une attente sur l'horloge réelle interdite (ADR-017) | Lint | Mineur | Aide `Events` sur un runtime tokio et `Clock::sleep` | Lint `disallowed-methods` |
| DEF-07 | 2026-10-25 | T02.03 | `ndaal-runtime` (banc) | Le banc d'aller-retour se bloquait : les deux cœurs pouvaient prendre le même rôle | Exécution (banc) | Important | Rôles choisis par numéro de cœur | Le banc lui-même, joué en CI |
| DEF-08 | 2026-10-25 | T02.03 | `ndaal-runtime` | Une requête abandonnée pendant l'arrêt du runtime était signalée `Dropped` (un bogue) au lieu de `ShuttingDown` | Test (échec) | Mineur | L'abandon pendant l'arrêt devient `ShuttingDown` | Contrat de la messagerie |
| DEF-09 | 2026-10-25 | T02.03 | `ndaal-runtime` (tests) | Un test de famine dépendait de l'ordonnancement des threads et échouait par intermittence | Test intermittent | Mineur | Test déterministe sur un seul thread | Ce test |
| DEF-10 | 2026-10-25 | T02.01 | Dépendances | Avis de sécurité RustSec sur la version de `tokio` | `cargo deny` | Important | Version de `tokio` relevée | `check-deny.sh` |
| DEF-11 | 2026-10-27 | T02.05 | `ndaal-io::sched` (tests) | Tests mal dimensionnés : une seule opération par classe (pas une saturation), trop peu de fenêtres pour la convergence | Test (échec) | Mineur | Tests corrigés | `weighted_shares`, `idle_time_does_not_lower_the_bandwidth` |
| DEF-12 | 2026-10-27 | T02.06 | `ndaal-io` (tests du pool) | Le test d'arrêt pouvait arrêter le pool avant le démarrage de la première opération | Test intermittent | Mineur | L'opération bloquante signale son démarrage | `shutdown_fails_waiting_ops` |
| DEF-13 | 2026-10-27 | T02.06 | `ndaal-io::real::IoPool` | Le répartiteur lisait les demandes avant les fins d'opération : une demande déposée avant une fin pouvait manquer la place libérée | Revue de l'auteur | Mineur | Fins d'opération lues avant les demandes | `high_overtakes_queued_low` (devenu déterministe) |
| DEF-14 | 2026-10-28 | T02.06 | `ndaal-io::real::IoPool` | Des opérations `Background` bridées par leur seau remplissaient la file du répartiteur : une opération `Critical` (le journal) n'était plus lue | Revue finale | Important | Une file par classe et une borne par classe | `critical_is_never_stuck_behind_background` |
| DEF-15 | 2026-10-28 | T02.05 | `ndaal-io::sim::SimDevice` | Une opération abandonnée (délai, coupure simulée) gardait sa place pour toujours : le périphérique se bloquait après `depth` abandons | Revue finale | Important | Garde qui rend la place | `a_dropped_operation_frees_its_slot` |
| DEF-16 | 2026-10-28 | T02.06 | `ndaal-io::real::IoPool` | À l'arrêt, la jauge `ndaal_io_inflight` n'était pas mise à jour et les fins d'opération en cours n'étaient pas comptées | Test intermittent | Mineur | Le répartiteur attend les opérations en cours | `metrics_are_exposed` |
| DEF-17 | 2026-10-28 | T02.07, T02.10 | Format du journal | La règle de FORMATS §2.3 (trame valide après une trame invalide = corruption) déclarait corrompue une partition après une coupure ordinaire : les pages d'une écriture atteignent le disque dans le désordre | Revue de conception | Important | Zone déchirée bornée (ADR-033) | `torn_zone_is_the_end`, `corruption_beyond_the_torn_zone` |
| DEF-18 | 2026-11-01 | T02.10 | `ndaal-wal::FileReader` | La recherche après une trame abîmée pouvait coûter des téraoctets de CRC sur des octets hostiles, et une trame isolée lointaine faisait conclure à une corruption | Revue finale | Important | Fenêtre de recherche bornée à une trame après 2 Mio | `scan_stops_one_fragment_past_the_torn_zone` |
| DEF-19 | 2026-11-01 | T02.08 | ADR-033 | L'invariant de l'écrivain était mal formulé (fin de vidage sur une trame, page réécrite comprise) et la perte silencieuse possible sous-estimée (3 Mio, pas 2) | Revue finale | Important | ADR-033 et critère de T02.08 précisés | Aucun (documentation) |
| DEF-20 | 2026-11-01 | T02.09 | Format du journal | Époque sur 16 bits dans la trame (une trame d'une ancienne vie pouvait sembler valide), trame recopiée d'un autre fichier non reconnue, coût de la recherche (P-59, P-60) | Revue finale | Important | CRC d'en-tête et CRC ensemencées (ADR-034) | `other_epoch_is_rejected_even_with_same_low_bits`, `frame_copied_from_another_file_is_rejected`, `header_crc_rejects_before_payload_crc` |
| DEF-21 | 2026-11-01 | T02.09 | `ndaal-wal::WalWriter` (reprise) | La reprise partait du dernier fichier numéroté, même vide (préparé d'avance) : le fichier qui contenait les données n'était ni vérifié ni effacé après sa fin | Test (échec) | Important | Reprise depuis le dernier fichier qui contient des trames | `open_refuses_a_corrupt_log` |
| DEF-22 | 2026-11-01 | T02.11 | `ndaal-io::sim::SimDisk` | Une écriture déchirée ne gardait qu'un préfixe de secteurs : la simulation ne produisait jamais le désordre contre lequel protègent l'ADR-033 et l'effacement à la reprise | Test par mutation | Important | Secteurs gardés ou perdus un par un | `torn_writes_keep_any_subset_of_sectors` ; le mutant « reprise sans effacement » est détecté |
| DEF-23 | 2026-11-01 | T02.11 | `ndaal-io::sim::DeviceDisk` | L'écriture et la synchronisation d'un vidage avaient lieu au même instant : une coupure ne pouvait jamais déchirer un vidage en simulation | Test par mutation | Important | Octets dans le cache de pages au dépôt, durables à la fin du service | `resume_never_resurrects_a_torn_flush` |
| DEF-24 | 2026-11-01 | T02.11 | `ndaal-sim` (tests) | Le test de reprise attendait une position impossible et se bloquait quand tout était vidé avant la coupure | Simulation (graine 10) | Mineur | Attente de la dernière position ajoutée | Ce test |
| DEF-25 | 2026-11-01 | T02.09 | `ndaal-wal::WalWriter` (reprise) | Un fichier de journal tronqué par un tiers faisait paniquer le premier ajout après la reprise | Revue finale | Important | Fichier dont la longueur n'est pas un multiple de 4 Kio refusé comme corrompu | `truncated_file_is_refused_without_panic` |
| DEF-26 | 2026-11-01 | T02.08 | `ndaal-wal::WalWriter` | Un enregistrement plus grand que la borne d'attente recevait `Busy` pour toujours | Revue finale | Important | Accepté quand rien d'autre n'attend | `oversized_record_is_accepted_when_alone` |
| DEF-27 | 2026-11-01 | T02.08 | `ndaal-wal::Flusher` | Les erreurs d'E/S du vidage n'étaient ni journalisées ni comptées | Revue finale | Important | `tracing::error!` et `ndaal_wal_flush_errors_total` | `failed_sync_fails_waiters_and_appends` |
| DEF-28 | 2026-11-01 | T02.11 | `ndaal-sim::wal_crash` | Le contrôle `Async` ne pouvait jamais échouer (coupure avant la fin de la fenêtre), et le scénario n'exerçait ni basculement, ni fragments, ni recyclage | Revue finale | Important | Scénario renforcé ; témoins « sync mensonger » et « vidage paresseux » | `wal_crash_detects_a_lazy_async_flush`, `the_scenario_exercises_what_it_checks` |
| DEF-29 | 2026-11-01 | T02.11 | `ndaal-sim::wal_crash` | La marge `Async` de 10 ms était choisie au jugé ; le scénario renforcé l'a contredite (graine 35) | Simulation (graine 35) | Mineur | Marge calculée à partir du modèle de périphérique (25 ms) | `wal_crash_loses_only_what_modes_allow` |
| DEF-30 | 2026-11-02 | T02.12 | `ndaal-storage::Partition` | Un journal en échec pendant un `submit` en mode `Sync` rendait une erreur différente de `wait_durable` et ne faisait pas passer la partition en échec (ni compteur, ni réveil des attentes) | Revue finale | Important | Le journal en échec fait passer la partition en `Failed` | `log_failure_during_sync_fails_the_partition` |
| DEF-31 | 2026-11-02 | T02.12 | `ndaal-storage::Partition` | La cause d'un échec d'application était jetée : ni journalisée, ni conservée | Revue finale | Important | `tracing::error!` avec la partition, le LSN et le type ; issue « inconnue » documentée sur `Failed` | Aucun (journalisation) ; le compteur est vérifié par `apply_failure_fails_the_partition` |
| DEF-32 | 2026-11-02 | T02.12 | `ndaal-storage::Partition` | `resume(Lsn(u64::MAX))` puis `submit` paniquait (en debug) ou revenait à `Lsn(0)` (en release) : la valeur vient du disque | Revue finale | Important | `Lsn::checked_next` ; la partition passe en échec | `lsn_overflow_fails_instead_of_panicking` |
| DEF-33 | 2026-11-02 | T02.12 | `ndaal-storage::Partition` | Un échec d'enregistrement des métriques était rapporté comme une erreur d'E/S du journal, cause d'origine perdue | Revue finale | Important | Variante `PartitionError::Metrics` qui garde la cause | Aucun (chemin non provocable avec un registre neuf) |
| DEF-34 | 2026-11-02 | T02.12 | `ndaal-storage::Partition` | La jauge `ndaal_wal_durable_lsn` n'avançait que si quelqu'un lisait le LSN durable | Revue finale | Important | Mise à jour à chaque `submit` | `durable_gauge_follows_without_a_reader` |
| DEF-35 | 2026-11-03 | T02.13 | `ndaal-storage::segment` | L'écrivain acceptait un en-tête avec `min_lsn > max_lsn`, puis écrivait et synchronisait un segment que le lecteur refuse : publié par le manifeste, il deviendrait illisible | Revue finale | Important | `create` refuse cet en-tête (`Misuse`) | `header_the_reader_refuses_is_refused_at_create` |
| DEF-36 | 2026-11-08 | T02.14 | `ndaal-common::keys` | `get_escaped` rendait `UnexpectedEnd { remaining: 0 }` quelle que soit l'entrée : le message d'erreur donnait une valeur fausse | Revue finale | Mineur | `remaining` = taille de l'entrée | `errors_tell_what_is_left` |
| DEF-37 | 2026-11-08 | T02.14 | `fuzz/corpus/README.md` | Un octet NUL littéral écrit dans le README par un script (` ` interprété) ; `check-hygiene.sh` ne l'a pas vu | Revue finale | Mineur | Texte `0x00` à la place de l'octet ; faille d'hygiène reportée (P-66) | Aucun |
