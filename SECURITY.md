# Politique de sécurité

## Signaler une vulnérabilité

**Ne signalez jamais une vulnérabilité dans un ticket public.**

Utilisez le signalement privé de GitHub : onglet **Security** du dépôt, puis **Report a vulnerability** (<https://github.com/LordPhenixDeNetra/Ndaal/security/advisories/new>). Seuls les responsables du projet voient le signalement.

Indiquez si possible :

- la version ou le commit concerné ;
- le composant touché (RESP, S3, API HTTP, réplication, stockage...) ;
- les étapes pour reproduire, ou une preuve de concept ;
- l'impact estimé (lecture ou modification de données, déni de service, élévation de droits...).

## Ce qui se passe ensuite

| Étape | Délai |
|---|---|
| Accusé de réception | 3 jours ouvrés au plus |
| Première évaluation (gravité, versions touchées) | 10 jours ouvrés au plus |
| Correctif et publication coordonnée | Selon la gravité ; la date de divulgation est convenue avec vous |

Vous êtes cité dans l'avis de sécurité publié, sauf si vous préférez rester anonyme.

## Versions prises en charge

Aucune version n'est publiée pour l'instant. Dès la première publication, ce tableau indiquera les versions qui reçoivent des correctifs de sécurité.

## Hors périmètre

- Les déploiements qui exposent Ndaal sur une interface publique sans authentification, alors que la configuration par défaut l'interdit ([CAHIER_DES_CHARGES.md](CAHIER_DES_CHARGES.md), ENF-SEC-04).
- Les vulnérabilités des dépendances déjà publiées et corrigées en amont : ouvrez un ticket public demandant la mise à jour.
