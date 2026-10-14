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

## À venir

- `compare/` : bancs comparatifs contre Valkey, Qdrant et MinIO (T01.18).
