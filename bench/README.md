# Bancs d'essai

Méthode et liste des bancs : [STRATEGIE_DE_TESTS.md §9](../STRATEGIE_DE_TESTS.md#9-bancs-dessai-de-performance).

## Micro-bancs (`criterion`)

- Ils vivent dans le dossier `benches/` de leur crate (par exemple [`crates/ndaal-common/benches/codec.rs`](../crates/ndaal-common/benches/codec.rs)).
- `bench/micro.sh` les lance tous et écrit leurs résultats au format `bencher`.
- Chaque nuit, le workflow `bench.yml` lance `micro.sh` et ajoute les résultats à l'historique de la branche `bench-data`. Le graphe est publié avec le site de documentation, sous `/bench/micro/`.
- La CI lance chaque banc une fois (`cargo bench -- --test`) pour qu'ils restent fonctionnels.

En attendant les machines de référence (D-05), les mesures nocturnes tournent sur les machines partagées de GitHub : elles indiquent une tendance, sans alerte de régression (P-42).

En local :

```bash
cargo bench -p ndaal-common            # rapport détaillé dans target/criterion/
bench/micro.sh                          # tous les micro-bancs, format bencher
```

## Bancs comparatifs (`compare/`)

Méthode et règles d'équité : [STRATEGIE_DE_TESTS.md §9.4](../STRATEGIE_DE_TESTS.md#94-bancs-comparatifs-parité-avec-les-services-remplacés). Tolérance : [ADR-027](../DECISIONS.md#adr-027--parité-de-performance-avec-les-services-remplacés).

```bash
bench/compare/kv.sh --duration 60 --repeats 5 --out bench-results/kv   # passages (Docker)
cargo run -p xtask -- compare bench-results/kv                        # verdict et report.md
```

- Options de `kv.sh` : `--server-cpus`, `--client-cpus`, `--reference`, `--candidate`, `--candidate-cpu-quota` (bride le candidat, pour simuler un écart).
- `compare` sort avec le code 0 dans la tolérance, 1 au-delà, 2 si les résultats sont invalides.
- Le workflow `compare.yml` tourne chaque nuit. Le lancer à la main avec `simulate_regression` doit ouvrir le ticket « compare: kv beyond tolerance ».
- Pour l'instant, les deux côtés sont Valkey (`valkey.conf`) : Ndaal devient le candidat avec son frontal RESP (J03).
