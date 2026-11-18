# Sécurité

Aether s'exécute avec les droits administrateur. Les failles de sécurité sont donc prises très au sérieux.

## Signaler une faille

Ne publiez pas la faille dans une issue publique. Utilisez la fonction **« Report a vulnerability »** (onglet *Security* du dépôt GitHub) pour un signalement privé.

Indiquez :

- la version d'Aether ;
- la version de Windows ;
- les étapes pour reproduire le problème ;
- l'impact possible.

## Périmètre

Sont notamment concernés :

- l'exécution de commandes non prévues ;
- la suppression de fichiers hors des dossiers temporaires prévus ;
- une modification du système qui ne peut pas être annulée ;
- toute atteinte à WebView2.
