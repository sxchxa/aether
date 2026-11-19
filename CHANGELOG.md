# Journal des versions

## 1.1.1

- **Transparence de « Tout optimiser »** : avant d'agir, une fenêtre liste chaque action, sans exception (réglages modifiés, fichiers nettoyés avec leur taille, purge de la RAM, point de restauration de sécurité). Chaque ligne indique le menu où la faire à la main ; un clic y mène directement. Rien n'est lancé sans confirmation.
- Même détail pour les corrections groupées de l'analyse (« N réglage(s) sans risque », « fichiers inutiles »).
- « Tout optimiser » n'applique plus que des réglages présents dans un menu, et ne nettoie que les catégories qui contiennent réellement des fichiers.

## 1.1.0

### Pour tous
- **Niveau au premier lancement** : Débutant (réglages sans danger uniquement), Joueur, Expérimenté. Modifiable dans Réglages et dans le menu latéral.
- **Points de restauration** : création automatique avant la première modification de la journée, page dédiée pour voir les points et revenir à l'un d'eux.
- **Aide** : les problèmes fréquents et leur solution, avec recherche et accès direct au bon endroit de l'application.
- Fenêtres « ? » plus claires : « Ce que ça fait », « Ce que vous y gagnez », « À savoir », et la commande exacte affichée en bas.
- **9 profils** rangés par usage (Quotidien, Jeu, Travail, Prestataire) : nouveaux profils Silencieux, Compétitif, Streaming, Bureautique et Création.
- **Signaler un problème** depuis Réglages (journal joint anonymisé).
- **Rapport PDF** : modifications appliquées, mesures avant / après et gains possibles.
- Installateur : choix du lancement au démarrage de Windows.
- **Mises à jour en un clic** : au démarrage, une fenêtre propose la nouvelle version ; « Mettre à jour maintenant » télécharge, installe et relance Aether automatiquement.

### Jeu et mémoire
- Services regroupés selon la mémoire installée (valeur calculée pour chaque PC : 8, 16, 32 Go…).
- Priorité élevée pour les jeux (planificateur multimédia), bouton Xbox sans Game Bar, diffusion de Windows désactivée, plein écran exclusif (niveau Expérimenté), gestionnaire des cartes hors connexion arrêté.

### Fiabilité
- Restauration exacte des réglages à plusieurs valeurs dont une partie était déjà optimale.
- Mesures avant / après plus stables (médiane de trois relevés).

## 1.0.1

- Jeux : un même jeu n'apparaît plus en double (bibliothèque Steam lue deux fois quand son chemin était écrit de deux façons).
- Clarté : chaque réglage dont la case est cochée décrit désormais l'état obtenu (ex. « Spouleur d'impression arrêté », « Réveil par paquet magique désactivé »), dans les 5 langues.

## 1.0.0 — première version publique

### Interface
- Refonte « premium dark » : nouveau logo, menu latéral, tableau de bord (score de santé, mesures en direct, historique des performances, dernière analyse).
- 5 langues : français, anglais, espagnol, allemand, italien.
- Écran de chargement avec progression réelle, palette de commandes Ctrl+K, panneau d'explication pour chaque réglage.

### Optimisation
- 76 réglages sûrs, tous réversibles et testés : services, jeux, interface, effets visuels, système de fichiers, alimentation, réseau (DNS chiffré, LLMNR, SMBv1), confidentialité, Windows Update.
- Profils guidés (dont « Client » : réglages sûrs uniquement, aucun service désactivé, nettoyage des fichiers temporaires), « Tout optimiser » avec point de restauration automatique, mesures Avant / Après, rapport HTML.

### Jeux
- Détection Steam, Epic Games et GOG. Boost au lancement, réglages par jeu, Boost automatique des jeux lancés hors d'Aether.

### Maintenance
- Nettoyage rapide et parallèle des fichiers temporaires et caches, analyseur de gros fichiers, entretien automatique hebdomadaire, réparation SFC et DISM.
- Bibliothèque de 69 applications installées via winget.

### Réseau et système
- Test de débit intégré (latence, téléchargement, envoi), comparatif DNS limité à Cloudflare, Google et Quad9.
- Passage de Windows 10 à Windows 11 en un clic : Assistant d'installation officiel sur les PC compatibles, image officielle de Microsoft sur les autres (fichiers et applications conservés).
- Processeur et mémoire affichés en moyennes sur 10 secondes, historique sur 10 min, 30 min et 1 h.

### Diagnostics
- Santé des disques (SMART : verdict, température, usure, erreurs), impact réel des programmes au démarrage, état de la batterie et rapport complet, recherche de fichiers en double (contenu identique).

### Confort
- Thème clair, sombre ou celui de Windows, contraste renforcé, raccourcis clavier (Alt+1 à Alt+8, Ctrl+,), prise en charge du contraste élevé de Windows.
- Option « Mode sombre de Windows », « Tout rétablir », lancement au démarrage, Boost automatique, notification de l’entretien automatique.

### Sécurité
- Audit complet : nettoyage limité aux dossiers prévus, protections propres aux PC portables, aux PC d'entreprise et à OneDrive, mises à jour de sécurité d'Edge préservées, WebView2 jamais touché.
