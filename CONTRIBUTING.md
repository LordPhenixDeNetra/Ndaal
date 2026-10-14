# Contribuer à Ndaal

Merci de votre intérêt pour Ndaal. Ce guide résume ce qu'on attend d'une contribution ; les règles complètes sont dans [CLAUDE.md](CLAUDE.md), qui s'applique aux personnes comme aux agents.

## Avant de commencer

- Lisez le [README](README.md), puis la partie de la documentation qui concerne votre travail ([ARCHITECTURE.md](ARCHITECTURE.md), [API_ET_PROTOCOLES.md](API_ET_PROTOCOLES.md), [FORMATS_DE_DONNEES.md](FORMATS_DE_DONNEES.md)).
- Rattachez votre travail à une tâche `Tnn.mm` du [cahier des tâches](CAHIER_DES_TACHES.md) et aux exigences qu'elle couvre. Pour un travail qui n'y figure pas, ouvrez d'abord un ticket.
- Une vulnérabilité ne se signale **jamais** dans un ticket public : voir [SECURITY.md](SECURITY.md).

## Certificat d'origine (DCO)

Chaque commit DOIT être signé avec `git commit -s`, qui ajoute la ligne :

```
Signed-off-by: Votre Nom <votre@adresse>
```

Cette ligne vaut engagement selon le [Developer Certificate of Origin](https://developercertificate.org/) : vous avez le droit de soumettre ce code et vous acceptez qu'il soit diffusé sous la licence du projet ([Apache 2.0](LICENSE)). Une demande de fusion contenant un commit non signé est refusée par la CI.

Pour ne pas publier votre adresse personnelle, utilisez l'adresse anonyme fournie par GitHub (*Settings → Emails*).

## Messages de commit

Format [Conventional Commits](https://www.conventionalcommits.org/), en anglais, avec la portée de la crate et la tâche :

```
feat(kv): active expiration with a timing wheel (T03.07)
fix(wal): reject a segment whose checksum does not match (T02.05)
```

Un commit = une intention. Pas de commit qui mélange une correction et un reformatage.

## Tests

- **Toute correction de bogue est accompagnée d'un test qui échouait avant la correction** (ENF-QUAL-04), y compris la graine de simulation qui l'a révélé.
- Toute nouvelle logique s'écrit test d'abord : on voit le test échouer, puis on écrit le code ([CLAUDE.md §5](CLAUDE.md#5-tests)).
- Ne désactivez, n'ignorez ni n'affaiblissez jamais un test pour faire passer la CI.

### Fuzzing

- Une cible = une fonction de [`crates/ndaal-fuzz`](crates/ndaal-fuzz/src/lib.rs) (inscrite dans `TARGETS`), un fichier `fuzz/fuzz_targets/<cible>.rs` qui l'appelle, et un corpus de départ dans `fuzz/corpus/<cible>/`, décrit dans [`fuzz/corpus/README.md`](fuzz/corpus/README.md).
- Lancer une cible : `./scripts/fuzz.sh <cible> [secondes]`. Sous Windows, le script passe par Docker.
- Une entrée qui a révélé un bogue est ajoutée au corpus avec la correction : c'est son test de non-régression.

### Bancs et documentation

- Micro-bancs : `cargo bench -p <crate>` ; voir [`bench/README.md`](bench/README.md).
- Site de documentation : `./scripts/build-docs.sh` (il demande `mdbook`, installé par `./scripts/install-tools.sh mdbook`), puis ouvrir `docs/book/html/index.html`. Les documents restent à la racine du dépôt ; `docs/book/` est généré.

## Avant d'ouvrir une demande de fusion

Lancez la liste de vérifications de [CLAUDE.md §11](CLAUDE.md#11-avant-de-déclarer-une-tâche-terminée), la seule tenue à jour : formatage, clippy, tests (`cargo nextest`), `cargo deny`, hygiène, liens de la documentation, vérification Linux (obligatoire sous macOS et Windows) et signature DCO. Les outils s'installent aux versions figées du projet avec `./scripts/install-tools.sh`. La CI relance tout cela sur chaque demande de fusion, sur Linux, macOS et Windows.

Dans la description de la demande de fusion, indiquez ce que vous avez lancé et le résultat, y compris ce qui a échoué ou n'a pas pu être lancé.

## Code de conduite

Ce projet applique le [Contributor Covenant](CODE_OF_CONDUCT.md). En participant, vous vous engagez à le respecter.
