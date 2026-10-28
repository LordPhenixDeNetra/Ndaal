# Messages de la ligne de commande ndaal (ADR-029). Clés : cli-*.
# Chaque clé de l'anglais existe ici (vérifié par les tests).

cli-about = Ndaal : stockage clé-valeur, recherche vectorielle et stockage d'objets dans un seul moteur.
cli-help-usage = Utilisation
cli-help-commands = Commandes
cli-help-options = Options

cli-value-file = FICHIER
cli-value-lang = LANGUE
cli-value-setting = CLE=VALEUR
cli-value-dir = DOSSIER

cli-opt-config = Fichier de configuration (TOML)
cli-opt-lang = Langue des messages : en, fr
cli-opt-set = Fixer un paramètre de configuration : section.cle=valeur
cli-opt-data-dir = Dossier des données (node.data_dir)
cli-opt-help = Afficher cette aide
cli-opt-version = Afficher la version

cli-cmd-server = Démarrer un nœud
cli-cmd-admin = Administrer un nœud ou une grappe
cli-cmd-backup = Sauvegarder un nœud
cli-cmd-restore = Restaurer une sauvegarde
cli-cmd-bench = Mesurer les performances
cli-cmd-config = Examiner la configuration
cli-cmd-config-reference = Lister chaque paramètre avec son type, sa valeur par défaut et sa variable
cli-cmd-config-show = Afficher la configuration effective
cli-cmd-doctor = Vérifier l'installation

cli-error-unknown-command = Commande inconnue : { $name }
cli-error-unknown-option = Option inconnue : { $name }
cli-error-invalid-value = Valeur absente ou invalide pour { $name }
cli-error-usage = Ligne de commande invalide
cli-error-hint = Lancez ndaal --help pour la liste des commandes et des options.
cli-cmd-health = Vérifier qu'un nœud est prêt (contrôle de santé des conteneurs)
cli-data-dir-error = Impossible de créer le dossier de données { $path } : { $error }
cli-listen-error = Impossible d'écouter sur { $addr } : { $error }
cli-health-not-ready = Le nœud { $addr } n'est pas prêt : { $reason }
cli-health-unreachable = connexion impossible ({ $error })
cli-health-timeout = pas de réponse en 5 secondes
cli-health-status = statut HTTP { $status }
cli-health-invalid = la réponse n'est pas du HTTP
cli-not-available = La commande { $command } n'est pas encore disponible.
cli-set-syntax = Valeur invalide pour --set : { $value } (attendu : section.cle=valeur).
cli-config-read-error = Impossible de lire le fichier de configuration { $path } : { $error }
cli-config-too-large = Le fichier de configuration { $path } dépasse { $max } octets.
cli-config-not-utf8 = Le fichier de configuration { $path } n'est pas du texte UTF-8.
cli-config-syntax = Fichier de configuration invalide (TOML) : { $error }
cli-config-unknown = Clé de configuration inconnue : { $key } (source : { $origin }).
cli-config-invalid = Valeur invalide pour { $key } (source : { $origin }) : attendu { $expected }.
cli-origin-default = valeurs par défaut
cli-origin-file = fichier de configuration
cli-origin-env = environnement
cli-origin-command-line = ligne de commande
cli-expected-path = un chemin non vide
cli-expected-text = un texte
cli-expected-choice = l'une des valeurs { $choices }
cli-expected-addr = une adresse comme 127.0.0.1:8080
cli-expected-log-level = un niveau ({ $levels }) ou une liste cible=niveau
cli-expected-bool = true ou false
cli-expected-duration = une durée comme 0, 250us, 10ms ou 2s (10s au plus)
cli-expected-count = auto ou un nombre de 0 à 4096
cli-expected-ratio = un nombre de 0 à 1 avec 6 décimales au plus, comme 0.3
cli-expected-range = un entier de { $min } à { $max }
cli-config-public-without-auth = { $key } (source : { $origin }) écoute sur une adresse non locale sans authentification. Fixez auth.allow_insecure = true seulement si c'est voulu.
cli-expected-table = une table [section]
cli-expected-scalar = une valeur simple, ni liste ni table
cli-runtime-error = Démarrage impossible : { $error }
