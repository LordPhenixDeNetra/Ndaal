# Glossaire

Termes utilisés dans la documentation de Ndaal, par ordre alphabétique.

| Terme | Définition |
|---|---|
| **ACL** | *Access Control List*. Liste des permissions accordées à un utilisateur ou à un rôle. |
| **ADR** | *Architecture Decision Record*. Fiche courte qui consigne une décision d'architecture, son contexte, les options étudiées et ses conséquences. Voir [DECISIONS.md](DECISIONS.md). |
| **ANN** | *Approximate Nearest Neighbors*. Recherche approximative des plus proches voisins : on accepte de manquer quelques voisins exacts en échange d'une recherche beaucoup plus rapide. |
| **Classe de charge** | Catégorie de travail ordonnancée séparément : `kv` (clé-valeur), `vector` (calcul vectoriel), `transfer` (transferts d'objets), `background` (compactage, réplication, maintenance). |
| **Code d'effacement** | *Erasure coding*. Technique qui découpe une donnée en *k* fragments de données et *m* fragments de parité ; n'importe quels *k* fragments parmi les *k+m* suffisent à la reconstruire. Exemple : 8+3 tolère la perte de 3 fragments avec un surcoût de 37,5 %. |
| **Compactage** | Opération de fond qui fusionne des segments, supprime les versions obsolètes et les éléments supprimés, et réécrit des segments plus compacts. |
| **Compartiment** | *Bucket*. Conteneur d'objets dans l'API S3. Dans Ndaal, un compartiment appartient à un espace de noms. |
| **Désagrégé (mode)** | Mode où le stockage objet est la source durable de vérité et où les nœuds de calcul ne gardent qu'un journal récent et des caches. |
| **DiskANN / Vamana** | Famille d'algorithmes d'index vectoriel en graphe conçus pour vivre principalement sur SSD, avec des vecteurs compressés en RAM. |
| **Domaine de panne** | Ensemble d'éléments pouvant tomber en panne ensemble (disque, nœud, baie, zone). Les réplicas et les fragments sont répartis sur des domaines de panne différents. |
| **Espace de noms** | *Namespace*. Unité d'isolation logique de Ndaal. Contient des clés, des collections vectorielles et des compartiments. C'est aussi l'unité de transaction et de placement. |
| **ETag** | Identifiant de version d'un objet S3, en général dérivé d'une empreinte MD5. |
| **Fsync** | Appel système forçant l'écriture physique des données sur le disque. |
| **Groupe de réplication** | Ensemble de réplicas (en général 3) qui partagent un même journal Raft et gèrent un ensemble de partitions. |
| **HNSW** | *Hierarchical Navigable Small World*. Index vectoriel en graphe multicouche, très performant en RAM. |
| **Intention** | *Intent*. Enregistrement provisoire dans le journal indiquant qu'une transaction distribuée prépare une modification, avant validation ou annulation. |
| **io_uring** | Interface Linux d'entrées/sorties asynchrones par files partagées entre le noyau et l'application, à très faible surcoût. |
| **Jalon** | Étape de construction du projet, regroupant des tâches et aboutissant à un résultat vérifiable. Ce n'est pas un numéro de version publié. |
| **Leader** | Réplica d'un groupe Raft qui reçoit les écritures et les réplique aux suiveurs. |
| **LSM** | *Log-Structured Merge tree*. Structure de stockage qui écrit en séquentiel dans des segments immuables triés, fusionnés par compactage. |
| **LSN** | *Log Sequence Number*. Numéro croissant attribué à chaque entrée du journal d'une partition. Sert de référence de version et de point de reprise. |
| **Manifeste** | Fichier décrivant l'état d'une partition : liste des segments actifs, leur niveau, le LSN couvert, le dernier point de contrôle. |
| **Memtable** | Structure en mémoire qui reçoit les écritures récentes avant leur écriture en segment. |
| **MVCC** | *Multi-Version Concurrency Control*. Conservation de plusieurs versions d'une donnée pour permettre des lectures cohérentes sans bloquer les écritures. |
| **Niveau de stockage** | *Tier*. Couche physique : RAM (chaud), NVMe local (tiède), stockage objet (froid et durable). |
| **Nœud** | Un processus Ndaal en cours d'exécution, en général sur une machine ou un conteneur. |
| **Objet provisoire** | Objet téléversé hors transaction, invisible tant qu'une transaction ne l'a pas validé, et supprimé automatiquement s'il ne l'est jamais. |
| **Groupe de cohérence** | Dans un espace de noms réparti, ensemble des données partageant la même clé de routage (par exemple `{doc:42}`) et donc placées dans la même partition. Les transactions y sont atomiques et sérialisables. |
| **Partition** | Unité logique de stockage, de journal (LSN propre), de réplication et de déplacement. Un espace de noms simple (mode `single`) tient dans une seule partition, qui peut en héberger plusieurs petits ; un espace de noms réparti (mode `sharded`) s'étend sur plusieurs partitions. |
| **Payload** | Charge utile JSON associée à un point vectoriel. |
| **Point** | Élément d'une collection vectorielle : identifiant + vecteur(s) + charge utile. |
| **Point de contrôle** | *Checkpoint*. État persistant à partir duquel la récupération peut reprendre sans relire tout le journal. |
| **PQ** | *Product Quantization*. Compression de vecteurs par découpage en sous-vecteurs remplacés par l'indice du centroïde le plus proche. |
| **Quantification** | Réduction de la précision des vecteurs (par exemple `f32` → `int8`) pour économiser mémoire et calcul. |
| **Raft** | Algorithme de consensus permettant à un groupe de réplicas de s'accorder sur un journal ordonné malgré des pannes. |
| **Rappel** | *Recall*. Proportion des vrais plus proches voisins effectivement retrouvés par une recherche approximative. Rappel@10 = vrais voisins parmi les 10 résultats / 10. |
| **Re-classement** | *Rescoring*. Recalcul du score des meilleurs candidats avec les vecteurs complets après une recherche sur vecteurs quantifiés. |
| **RESP** | *REdis Serialization Protocol*. Protocole texte/binaire des clients Redis (versions 2 et 3). |
| **Rôle de nœud** | Fonction assurée par un nœud : `gateway`, `data`, `vector`, `storage`, ou `all`. |
| **RRF** | *Reciprocal Rank Fusion*. Méthode de fusion de plusieurs classements utilisée en recherche hybride. |
| **Segment** | Fichier immuable contenant des données d'un seul type (clé-valeur, vecteurs, blocs d'objets) pour une partition et une plage de LSN. |
| **Shard** | Dans Ndaal, ensemble de partitions possédées par **un cœur CPU** sur un nœud. Toutes les écritures d'une partition passent par le thread de son shard. |
| **Shared-nothing** | Architecture dans laquelle chaque cœur possède ses propres données et ne partage pas de structure mutable avec les autres ; la communication se fait par messages. |
| **SigV4** | *AWS Signature Version 4*. Mécanisme de signature des requêtes S3. |
| **SST** | *Sorted String Table*. Segment immuable de paires clé-valeur triées par clé. |
| **Température** | Mesure de la fréquence d'accès récente d'une donnée, utilisée pour décider de son niveau de stockage. |
| **Thread-per-core** | Modèle où chaque cœur CPU exécute un seul thread qui possède ses données et sa boucle d'événements, sans verrous partagés. |
| **TTL** | *Time To Live*. Durée de vie d'une clé avant expiration automatique. |
| **WAL** | *Write-Ahead Log*. Journal dans lequel toute modification est écrite avant d'être appliquée, pour permettre la récupération après panne. Ndaal utilise un journal unique pour les trois modèles. |
