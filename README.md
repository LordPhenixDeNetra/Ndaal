# Ndaal — Moteur de données unifié (cache, vecteurs, objets)

Ndaal est un moteur de données écrit entièrement en **Rust** qui réunit dans un seul système trois services habituellement déployés séparément :

| Service remplacé | Rôle dans Ndaal | Protocole exposé |
|---|---|---|
| Redis / Valkey | Stockage clé-valeur en mémoire, cache, compteurs, TTL | RESP3 (compatible clients Redis) |
| Qdrant / Weaviate | Base vectorielle : stockage d'embeddings, recherche par similarité, filtres | HTTP/JSON et gRPC |
| MinIO / RustFS | Stockage objet compatible S3 : fichiers, médias, sauvegardes | API S3 (signature SigV4) |

L'idée centrale : **ces trois services ne sont pas trois bases différentes, mais trois manières d'accéder à une même hiérarchie de stockage** (RAM → NVMe → stockage objet). Ndaal gère cette hiérarchie une seule fois, avec un seul journal, un seul mécanisme de réplication et un seul outil d'exploitation. Il offre surtout ce qu'aucune combinaison de services séparés ne peut garantir : **des transactions atomiques entre les trois modèles de données**.

```
TRANSACTION (espace de noms « acme »)
  PUT    objet   docs/contrat.pdf
  UPSERT vecteur collection=documents id=42 embedding=[...] payload={"fichier": "docs/contrat.pdf"}
  DEL    clé     cache:recherche:*
COMMIT  → tout ou rien
```

---

## Organisation de la documentation

| Document | Contenu | Pour qui |
|---|---|---|
| [CAHIER_DES_CHARGES.md](CAHIER_DES_CHARGES.md) | Besoins, périmètre, exigences fonctionnelles et non fonctionnelles numérotées, critères d'acceptation | Tout le monde : c'est la référence de *ce que* l'on construit |
| [ARCHITECTURE.md](ARCHITECTURE.md) | Conception détaillée : couches, composants, flux, modèle de threads, stockage par niveaux, réplication, distribution | Développeurs : *comment* on le construit |
| [FORMATS_DE_DONNEES.md](FORMATS_DE_DONNEES.md) | Formats binaires : journal (WAL), segments, manifeste, encodage des clés, compatibilité | Développeurs du moteur de stockage |
| [API_ET_PROTOCOLES.md](API_ET_PROTOCOLES.md) | Sous-ensembles RESP et S3 supportés, API vectorielle, API transactionnelle, API d'administration, codes d'erreur | Développeurs d'applications et de SDK |
| [DECISIONS.md](DECISIONS.md) | Registre des décisions d'architecture (ADR) avec contexte, options et conséquences | Tout contributeur qui se demande « pourquoi ? » |
| [STRATEGIE_DE_TESTS.md](STRATEGIE_DE_TESTS.md) | Tests unitaires, propriétés, fuzzing, simulation déterministe, tests de compatibilité, bancs d'essai | Développeurs et relecteurs |
| [EXPLOITATION.md](EXPLOITATION.md) | Déploiement, configuration, supervision, sauvegarde, mise à jour, procédures d'incident | Opérateurs |
| [CAHIER_DES_TACHES.md](CAHIER_DES_TACHES.md) | Toutes les tâches, regroupées par jalons, avec dépendances, estimations et critères de fin : **le document de suivi** | Chef de projet et développeurs |
| [GLOSSAIRE.md](GLOSSAIRE.md) | Définitions des termes techniques utilisés | Tout le monde |
| [CLAUDE.md](CLAUDE.md) | Règles et bonnes pratiques de code, pour les développeurs et les agents | Tout contributeur, avant d'écrire du code |
| [POINTS_REPORTES.md](POINTS_REPORTES.md) | Décisions en attente et points remis à plus tard, avec leur échéance | Responsable du projet et contributeurs |

### Ordre de lecture conseillé

1. Ce README
2. [CAHIER_DES_CHARGES.md](CAHIER_DES_CHARGES.md)
3. [ARCHITECTURE.md](ARCHITECTURE.md)
4. [CAHIER_DES_TACHES.md](CAHIER_DES_TACHES.md)
5. Les autres documents au besoin

---

## Conventions de la documentation

### Pas de numéros de version dans la documentation de conception

Le projet est découpé en **jalons** (« Jalon 1 — Fondations », « Jalon 2 — Noyau de stockage », etc.). Un jalon est une étape de construction, **pas** une version publiée du logiciel. Les numéros de version publiés (semver) sont décidés au moment de la publication et ne figurent pas dans ces documents, afin d'éviter toute confusion entre étape de travail et version livrée.

Seule exception : le préfixe `/v1/` des chemins de l'API HTTP (et le paquet gRPC `ndaal.v1`) désigne la **version du contrat d'API**, une convention web standard qui n'a aucun lien avec les jalons ni avec les versions publiées du logiciel.

### Identifiants

| Préfixe | Signification | Exemple | Défini dans |
|---|---|---|---|
| `EF-xxx-nn` | Exigence fonctionnelle | `EF-KV-03` | Cahier des charges |
| `ENF-xxx-nn` | Exigence non fonctionnelle | `ENF-PERF-02` | Cahier des charges |
| `ADR-nnn` | Décision d'architecture | `ADR-004` | DECISIONS.md |
| `Jnn` | Jalon | `J05` | Cahier des tâches |
| `Tnn.mm` | Tâche *mm* du jalon *nn* | `T05.07` | Cahier des tâches |
| `R-nn` | Risque | `R-03` | Cahier des charges |

Chaque tâche référence les exigences qu'elle satisfait, et chaque exigence est couverte par au moins une tâche : cela permet de vérifier qu'aucun besoin n'est oublié.

### Termes normatifs

- **DOIT** : obligatoire.
- **DEVRAIT** : fortement recommandé ; un écart doit être justifié dans un ADR.
- **PEUT** : optionnel.

---

## Vue d'ensemble en une image

```
                     Applications (clients Redis, SDK S3, clients vectoriels, SDK Ndaal)
                                │                │                 │
                     ┌──────────▼────────────────▼─────────────────▼───────────┐
   Frontaux          │   RESP3          API S3           API vectorielle/Tx    │
   de protocoles     └──────────┬────────────────┬─────────────────┬───────────┘
                                └────────────────┼─────────────────┘
                     ┌───────────────────────────▼─────────────────────────────┐
   Cœur              │ Routage → Shard propriétaire (un cœur CPU) → Transaction │
                     │              Journal unifié (WAL) répliqué               │
                     └──────┬──────────────────┬──────────────────┬────────────┘
                     ┌──────▼──────┐    ┌──────▼──────┐    ┌──────▼──────┐
   Moteurs           │ Clé-valeur  │    │  Vecteurs   │    │   Objets    │
                     └──────┬──────┘    └──────┬──────┘    └──────┬──────┘
                     ┌──────▼──────────────────▼──────────────────▼──────┐
   Niveaux           │  RAM  ──►  NVMe local (cache)  ──►  Stockage objet │
   de stockage       │ (chaud)        (tiède)              (froid, durable)│
                     └────────────────────────────────────────────────────┘
```

Les détails sont dans [ARCHITECTURE.md](ARCHITECTURE.md).

---

## État du projet

Le suivi d'avancement est tenu dans [CAHIER_DES_TACHES.md](CAHIER_DES_TACHES.md), section « Tableau de bord ».

## Contribuer

Voir [CONTRIBUTING.md](CONTRIBUTING.md) (certificat d'origine, tests, vérifications) et le [code de conduite](CODE_OF_CONDUCT.md). Les vulnérabilités se signalent en privé : [SECURITY.md](SECURITY.md).

## Licence

[Apache 2.0](LICENSE) (voir [ADR-001](DECISIONS.md#adr-001--licence-apache-20)).

<!--


docker compose -f deploy/docker/compose.yaml up -d --build --wait
curl http://127.0.0.1:8080/health/ready
docker compose -f deploy/docker/compose.yaml down -v

 -->
