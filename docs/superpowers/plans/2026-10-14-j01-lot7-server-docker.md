# J01 lot 7 — Serveur minimal, santé, métriques et Docker : plan d'implémentation

> **Pour les agents :** sous-compétence requise : superpowers:executing-plans (ou subagent-driven-development), tâche par tâche.
>
> **Pas d'étape de commit** : le responsable commite sur une branche `j01-lot7-server-docker` et ouvre une demande de fusion ; la CI fait foi. Le compte rendu donne les commandes git **et le contenu du formulaire de demande de fusion**.

**Objectif :** `ndaal server` écoute en HTTP et répond sur `/health/live`, `/health/ready` et `/metrics` ; `ndaal health` interroge ce point ; une image Docker et `deploy/docker/compose.yaml` démarrent un nœud dont `/health/ready` répond (T01.22), vérifié en CI.

**Architecture :** `ndaal-io` ouvre les connexions TCP en flux d'octets (la seule couche qui touche au réseau, ADR-017) et fournit le signal d'arrêt et le dossier de données par défaut de chaque système. `ndaal-proto-http` construit le routeur `axum` et sert sur le flux de `ndaal-io`. `ndaal-cli` assemble : configuration, journaux, dossier de données, serveur HTTP, arrêt propre ; `ndaal health` est un client HTTP/1.1 minimal sur le même flux. L'image est construite en plusieurs étapes et exécute `ndaal server` sans privilèges.

**Pile :** `axum` 0.8 (sur `hyper` 1, HTTP/1.1 seulement), `tokio` (`signal`, `io-util`, `macros`), Docker et Compose.

**Spécification :** [CAHIER_DES_TACHES.md](../../../CAHIER_DES_TACHES.md#j01--fondations-du-projet) T01.22 ; [CAHIER_DES_CHARGES.md](../../../CAHIER_DES_CHARGES.md) EF-OPS-07, EF-OPS-12, ENF-SEC-04 ; [API_ET_PROTOCOLES.md §8](../../../API_ET_PROTOCOLES.md#8-api-dadministration) ; [EXPLOITATION.md](../../../EXPLOITATION.md) §1.2, §1.3, §3 ; [ARCHITECTURE.md §14.3](../../../ARCHITECTURE.md#143-http-et-grpc) ; [DECISIONS.md](../../../DECISIONS.md) ADR-003, ADR-017, ADR-030 ; [POINTS_REPORTES.md](../../../POINTS_REPORTES.md) P-27, P-34, P-36.

## Contraintes globales

- ENF-SEC-04 : écoute par défaut sur `127.0.0.1` ; une adresse d'écoute non locale sans authentification est **refusée** sauf `auth.allow_insecure = true` (nom de EXPLOITATION §3).
- Image (EXPLOITATION §1.2) : construction en plusieurs étapes, utilisateur non root **UID 10001**, système de fichiers en lecture seule hors du volume `/data`, port `8080`, contrôle de santé intégré, configuration par `/etc/ndaal/ndaal.toml` ou `NDAAL_*`. Images de base **épinglées par empreinte** (CLAUDE.md §4.12).
- Docker ne publie le port que sur `127.0.0.1` dans `compose.yaml` : le nœud n'est pas exposé hors de la machine.
- Toute attente a un délai : `ndaal health` abandonne après 5 s (`HEALTH_TIMEOUT`).
- Arrêt propre sur Ctrl-C **et** SIGTERM (en conteneur, le processus est le PID 1 : sans traitement de SIGTERM, `docker stop` attend 10 s puis le tue).
- Textes humains de la CLI par Fluent (`cli-*`), journaux en anglais ; codes de sortie : `0`, `1` (exécution, nœud non prêt), `2` (usage, configuration).
- Test d'abord ; mutation quand un test passe du premier coup.

## Décisions à valider avec le plan

1. **`axum` pour le frontal HTTP**, consigné dans un nouvel **ADR-032**. Il s'agit d'une dépendance structurante (CLAUDE.md §4.11) ; ARCHITECTURE §14.3 et ADR-003 citent déjà `hyper` et `axum`. Fonctionnalités : `http1` et `tokio` seulement. HTTP/2, `serde_json` et `tower-http` attendront les tâches qui en ont besoin.
2. **Flux TCP réels seulement dans `ndaal-io`** (`real::tcp`). Le réseau simulé actuel transporte des trames, pas des octets, donc le frontal HTTP ne tourne pas encore en simulation. Ce n'est pas gênant tant qu'il ne porte aucune logique : à inscrire en **P-37**, à régler avant l'API HTTP métier (J05 et J06).
3. **Image d'exécution `distroless/cc` (glibc), pas un binaire statique.** EXPLOITATION §1.2 dit « binaire statique » ; un binaire musl demanderait un allocateur externe (souvent en C, contraire à C-01) pour garder ses performances. L'image reste minimale : pas de shell, certificats racine, utilisateur non root. EXPLOITATION §1.2 sera corrigé dans la même modification.
4. **L'image seule refuse de démarrer.** Elle écoute sur `0.0.0.0:8080` (sinon le port publié serait injoignable) et n'active pas `auth.allow_insecure` : par ENF-SEC-04, `docker run` sans option s'arrête avec un message clair. `compose.yaml` fixe l'option explicitement et ne publie que sur `127.0.0.1:8080`.
5. **P-34, dossier de données par défaut selon le système**, fourni par `ndaal-io` (seule couche autorisée à utiliser `cfg(target_os)`) :
   - Linux : `/var/lib/ndaal` ;
   - macOS : `$HOME/Library/Application Support/ndaal` ;
   - Windows : `%LOCALAPPDATA%\ndaal\data` ;
   - variable absente : `ndaal-data`, relatif au dossier courant.

   `ndaal server` crée ce dossier au démarrage et n'est « prêt » qu'ensuite.
6. **Prêt = HTTP en écoute et dossier de données créé.** Les vraies conditions (journal relu, partitions ouvertes) viendront avec J02. Les réponses de santé sont du texte (`ok`) ; le format JSON de l'API viendra avec `serde_json`.
7. **CI** : une tâche `docker` (ubuntu-24.04) construit l'image, lance `docker compose up --wait` puis interroge `/health/ready`. GitHub n'offre pas de conteneurs Linux sur macOS et Windows : ces deux systèmes se vérifient localement avec Docker Desktop (critère de T01.22). **Vous** ajouterez `docker` aux vérifications obligatoires du jeu de règles, si vous le souhaitez.
8. **P-36** :
   - délai par test dans `.config/nextest.toml` : signalé après 30 s, tué après 120 s. Un test de processus bloqué échoue alors au lieu de bloquer la CI ;
   - exemption d'hygiène de `collections.rs` réduite au seul lint `clippy::disallowed_types`.

## Points à surveiller en revue

1. **`http.listen = "0.0.0.0:8080"` ou `"[::]:8080"` sans `auth.allow_insecure`** : refus (code 2) ; `127.0.0.2`, `::1` restent acceptés (bouclage). Testé à la tâche 2.
2. **`ndaal health` contre un port fermé, un serveur muet ou une réponse non HTTP** : code 1 en moins de 5 s, jamais de blocage. Testé à la tâche 4.
3. **`ndaal health` quand le nœud écoute sur `0.0.0.0` ou `[::]`** : il interroge le bouclage de la même famille, pas l'adresse « non spécifiée ». Testé à la tâche 4.
4. **Dossier de données impossible à créer** (fichier à la place, droits) : code 1, message traduit, `/health/ready` jamais vrai. Testé à la tâche 4.
5. **`docker stop`** : arrêt en moins de 10 s grâce à SIGTERM. Testé à la tâche 5 (CI : `docker compose stop -t 5` puis code de sortie du conteneur).

---

### Tâche 1 : `ndaal-io` — flux TCP, signal d'arrêt, dossier par défaut

**Fichiers :** Créer `crates/ndaal-io/src/real/tcp.rs`, `crates/ndaal-io/src/real/signal.rs`, `crates/ndaal-io/src/platform.rs`, `crates/ndaal-io/tests/real_tcp.rs` ; Modifier `crates/ndaal-io/src/real/mod.rs`, `crates/ndaal-io/src/lib.rs`, `crates/ndaal-io/Cargo.toml` (`tokio` : `signal`).

**Interfaces (produit) :**
- `pub struct TcpByteListener` : `pub async fn bind(addr: SocketAddr) -> io::Result<Self>`, `pub async fn accept(&self) -> io::Result<(ByteStream, SocketAddr)>`, `pub fn local_addr(&self) -> io::Result<SocketAddr>`.
- `pub struct ByteStream` : implémente `tokio::io::AsyncRead` et `AsyncWrite` (délégation), `Send`, et `pub async fn connect(addr: SocketAddr) -> io::Result<ByteStream>`.
- `pub async fn shutdown_signal() -> io::Result<()>` : se termine au premier Ctrl-C ou SIGTERM (`#[cfg(unix)]`, repli Ctrl-C seul ailleurs).
- `pub fn default_data_dir(var: impl Fn(&str) -> Option<String>) -> PathBuf` dans `ndaal_io::platform` (la lecture de l'environnement est passée en paramètre, pour être testable).

- [ ] **Tests d'abord :**
  - `bound_listener_echoes_bytes` : `bind("127.0.0.1:0")`, `connect(local_addr)`, écrire 100 000 octets et les relire en écho, puis vérifier que la fermeture côté client donne une lecture de 0.
  - `connect_to_a_closed_port_fails` : se lier, relever le port, libérer l'écouteur, puis vérifier que `connect` rend une erreur.
  - `default_data_dir_per_system` : sur la cible courante (`cfg!(target_os = ...)` dans le test), le résultat attendu avec `HOME` ou `LOCALAPPDATA` fourni, puis le repli `ndaal-data` sans variable.

---

### Tâche 2 : configuration — `auth.allow_insecure`, écoute publique, défaut par système

**Fichiers :** Modifier `crates/ndaal-common/src/config/{mod.rs, params.rs}`, `crates/ndaal-common/tests/config.rs`, `crates/ndaal-cli/src/{error.rs, commands.rs}`, `locales/{en,fr}/cli.ftl`.

**Interfaces (produit) :**
- `Kind::Bool` (`"true"`/`"false"`, ce que donnent aussi les booléens TOML).
- Paramètre `auth.allow_insecure` (`Bool`, défaut `"false"`, aide : « allow a non-loopback listen address without authentication (ENF-SEC-04) »).
- `Config.auth: AuthConfig { allow_insecure: bool }`.
- `ConfigError::PublicWithoutAuth { key: String, origin: Origin }` (`key` = `http.listen`).
- `Sources.defaults: Vec<(String, String)>` : défauts propres au système (le dossier de données), appliqués après ceux de `PARAMS` avec `Origin::Default`. La CLI y passe `("node.data_dir", default_data_dir(...))`.
- Clés Fluent : `cli-expected-bool`, `cli-config-public-without-auth`.

- [ ] **Tests d'abord :**
  - `public_listen_requires_allow_insecure` : `0.0.0.0:8080` et `[::]:8080` rendent `PublicWithoutAuth` ; `127.0.0.2:8080`, `[::1]:8080` et `0.0.0.0:8080` avec `auth.allow_insecure = true` (TOML booléen) sont acceptés.
  - `bool_values` : `NDAAL_AUTH__ALLOW_INSECURE=yes` donne `Invalid` avec `Expected::Value(Kind::Bool)`.
  - `platform_defaults_sit_between_params_and_file` : `defaults` remplace `/var/lib/ndaal`, et le fichier le remplace à son tour.
  - CLI : `--lang fr --set http.listen=0.0.0.0:1 config show` sort avec le code 2 et le message contient `auth.allow_insecure`.

---

### Tâche 3 : `ndaal-proto-http` — routeur de santé et de métriques

**Fichiers :** Créer `crates/ndaal-proto-http/src/{health.rs, listener.rs}`, `crates/ndaal-proto-http/tests/http.rs` ; Modifier `crates/ndaal-proto-http/src/lib.rs`, `crates/ndaal-proto-http/Cargo.toml`, `Cargo.toml` (`axum`).

**Interfaces :**
- Consomme : `TcpByteListener`, `ByteStream` (tâche 1) ; `ndaal_observe::metrics::Registry` (`render`, `counter`).
- Produit :
  - `pub struct NodeState` (`Clone`) : `pub fn new(registry: Registry) -> Self`, `pub fn set_ready(&self)` (un `AtomicBool` partagé ; c'est le seul état, hors du chemin critique).
  - `pub fn router(state: NodeState) -> axum::Router`.
  - `pub async fn serve(listener: TcpByteListener, router: axum::Router, shutdown: impl Future<Output = ()> + Send + 'static) -> io::Result<()>`, avec arrêt propre.
- Routes :
  - `GET /health/live` répond `200 ok` ;
  - `GET /health/ready` répond `200 ok` après `set_ready`, et `503 not ready` avant ;
  - `GET /metrics` répond `200`, `Content-Type: text/plain; version=0.0.4`, avec le rendu du registre ;
  - toute autre route répond `404`.
- Métrique : `ndaal_http_requests_total{route, status}`, où `route` vaut `/health/live`, `/health/ready`, `/metrics` ou `other`. Les chemins inconnus sont regroupés sous `other`, pour borner le nombre de séries.
- `listener.rs` : un type local qui enveloppe `TcpByteListener` et implémente `axum::serve::Listener`. Une erreur d'`accept` est journalisée, comptée dans `ndaal_http_accept_errors_total`, puis suivie d'une pause de 100 ms par `SystemClock` (jamais de boucle active).

- [ ] **Tests d'abord** (`tests/http.rs`, vrai serveur sur `127.0.0.1:0`, requêtes HTTP/1.1 brutes par `ByteStream`) :
  - `live_is_always_ok` ;
  - `ready_is_503_until_set_ready` ;
  - `metrics_are_prometheus_text`, qui vérifie l'en-tête et la présence de `ndaal_http_requests_total{route="/health/live",status="200"} 1` après un appel ;
  - `unknown_paths_are_404_and_grouped`, qui vérifie que deux chemins distincts donnent une seule série `route="other"` ;
  - `serve_returns_after_shutdown`.

---

### Tâche 4 : `ndaal server` et `ndaal health`

**Fichiers :** Créer `crates/ndaal-cli/src/health.rs`, `crates/ndaal-cli/tests/server.rs` ; Modifier `crates/ndaal-cli/src/{commands.rs, cli.rs, main.rs, error.rs}`, `crates/ndaal-cli/Cargo.toml`, `locales/{en,fr}/cli.ftl`.

**Interfaces :**
- Consomme : les tâches 1 à 3.
- `server` déroule, dans cet ordre :
  1. journaux ;
  2. gestion des paniques ;
  3. création du dossier de données (`RealDisk::create_dir_all`) ;
  4. `TcpByteListener::bind(http.listen)` ;
  5. événement de journal `"http listening"` avec le champ `addr`, l'adresse effective ;
  6. `set_ready`, puis service jusqu'à `shutdown_signal`.

  Une erreur de dossier ou de liaison donne le code 1 et un message traduit (`cli-data-dir-error`, `cli-listen-error`).
- `health` : `pub(crate) async fn check(addr: SocketAddr) -> Result<(), HealthError>`.
  - Il envoie `GET /health/ready HTTP/1.1` avec `Connection: close`, lit au plus 4 KiB et exige une ligne d'état `HTTP/1.1 200`.
  - Délai `HEALTH_TIMEOUT = 5 s` (`SystemClock::sleep` avec `tokio::select!`).
  - Une adresse non spécifiée (`0.0.0.0`, `[::]`) est remplacée par le bouclage de la même famille.
  - Résultats : code 0 et `cli-health-ready` si le nœud est prêt ; sinon code 1 et `cli-health-not-ready { $reason }`.
- Sous-commande `health` ajoutée à l'interface (`cli-cmd-health`).

- [ ] **Tests d'abord** (`tests/server.rs`, binaire réel ; l'adresse se lit dans la ligne JSON « http listening » de la sortie standard) :
  - `server_becomes_ready_and_health_succeeds` : `server --set http.listen=127.0.0.1:0 --data-dir <tmp>/d`, puis `health --set http.listen=<addr>` sort avec le code 0, et `<tmp>/d` existe.
  - `health_fails_fast_on_closed_port` : code 1 en moins de 5 s.
  - `health_times_out_on_silent_server` : un écouteur `TcpByteListener` qui accepte sans jamais répondre ; code 1 en environ 5 s.
  - `health_uses_loopback_for_unspecified_address`, testé unitairement sur la fonction de choix d'adresse.
  - `server_fails_when_data_dir_is_a_file` : code 1 et message traduit.
  - `public_listen_refused_without_allow_insecure` : code 2.

---

### Tâche 5 : image Docker, Compose, CI et documentation

**Fichiers :**
- Créer :
  - `deploy/docker/Dockerfile` ;
  - `deploy/docker/compose.yaml` ;
  - `.dockerignore` ;
  - `.config/nextest.toml`.
- Modifier :
  - `.github/workflows/ci.yml` (tâche `docker`) ;
  - `xtask/src/hygiene/rust.rs` (P-36) ;
  - `DECISIONS.md` (ADR-032) ;
  - `EXPLOITATION.md` §1.2 et §1.3 ;
  - `STRATEGIE_DE_TESTS.md` §13.4 ;
  - `CAHIER_DES_TACHES.md` (T01.22) ;
  - `POINTS_REPORTES.md` (P-27, P-34, P-36 clos ; P-37 ouvert).

**Dockerfile** :
- Construction : `rust@sha256:114c7a44…`, la même empreinte que `check-linux.sh`. Commande : `cargo build --release --locked -p ndaal-cli`, avec des caches BuildKit pour le registre et `target`.
- Exécution : `gcr.io/distroless/cc-debian12@sha256:…`, empreinte de l'index multi-architecture relevée à l'exécution.
- Contenu de l'image finale :
  - `COPY` du binaire vers `/usr/local/bin/ndaal` ;
  - `/data` vide, avec `--chown=10001:10001` (copié depuis l'étape de construction : l'image d'exécution n'a pas de shell) ;
  - `USER 10001:10001`.
- Variables : `ENV NDAAL_NODE__DATA_DIR=/data NDAAL_HTTP__LISTEN=0.0.0.0:8080`.
- `EXPOSE 8080`, `VOLUME /data`.
- `HEALTHCHECK --interval=10s --timeout=6s --start-period=10s CMD ["/usr/local/bin/ndaal", "health"]`.
- `ENTRYPOINT ["/usr/local/bin/ndaal"]`, `CMD ["server"]`.

**compose.yaml** :
- un service `ndaal` construit depuis la racine du dépôt ;
- `read_only: true` ;
- `environment: NDAAL_AUTH__ALLOW_INSECURE: "true"`, avec un commentaire qui renvoie à ENF-SEC-04 ;
- `ports: ["127.0.0.1:8080:8080"]` ;
- volume nommé `ndaal-data:/data` ;
- `stop_grace_period: 10s`.

**CI**, tâche `docker` (ubuntu-24.04) :
1. `docker compose -f deploy/docker/compose.yaml up -d --build --wait` ;
2. `curl -fsS http://127.0.0.1:8080/health/ready` ;
3. `docker compose stop -t 5`, en vérifiant que le conteneur sort avec le code 0 (arrêt sur SIGTERM et non par SIGKILL) ;
4. `docker compose down -v`.

- [ ] **Tests d'abord :**
  - la tâche CI elle-même, que l'on voit échouer avant le `HEALTHCHECK`, en local avec Docker si celui-ci répond ;
  - `check-workflows.sh` ;
  - un test d'hygiène : l'exemption accepte `#[allow(clippy::disallowed_types)]` dans `collections.rs`, mais refuse `clippy::disallowed_methods` et `clippy::all` dans ce même fichier ;
  - `.config/nextest.toml` vérifié par un test volontairement bloquant, lancé une fois en local puis retiré : il doit être tué au bout du délai.
- [ ] Chaîne complète de CLAUDE.md §11, dont `check-linux.sh` et `check-workflows.sh` (ils exigent Docker) ; revue finale par un relecteur neuf ; commandes git et contenu du formulaire de demande de fusion.
