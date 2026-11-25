# Journal des versions

Toutes les évolutions notables d'Ignite. Format inspiré de [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/), versions selon [SemVer](https://semver.org/lang/fr/).

## [1.5.1] - 2026-11-25

### Corrigé
- Fin de session détectée en quelques secondes : surveillance chaque seconde pendant une partie, délai de grâce réduit une fois le jeu bien lancé, et les rapports de plantage ou navigateurs intégrés restés ouverts dans le dossier du jeu ne prolongent plus la session. Le récap s'ouvre sans attendre l'analyse des captures.
- Riot : ouvrir le client Riot ne démarre plus une session Teamfight Tactics. TFT (client autonome) et 2XKO sont suivis par leur processus de jeu.

## [1.5.0] - 2026-11-13

### Ajouté
- Interface en anglais : choix de la langue dans Réglages › Général (français ou anglais, langue de Windows par défaut). Le menu de la zone de notification, les noms des succès et les infos venant de Steam suivent ce choix ; l'installeur s'affiche aussi en anglais sur un Windows anglais.
- Mises à jour automatiques : Ignite vérifie les releases GitHub au démarrage et propose d'installer la nouvelle version (Réglages › À propos pour vérifier à la main).

### Modifié
- Réglages rangés par catégories (Général, Sources, Émulateurs, Succès et suivi, Captures, Comptes et clés, Jeux masqués, À propos), une à la fois. Toutes les clés d'API sont réunies dans « Comptes et clés ».

### Corrigé
- Recherche : `genre:` et `joueurs:` acceptent les noms anglais et les formes courtes (`genre:adventure`, `players:coop`, `joueurs:multi`…). Avant, `joueurs:coop` ne trouvait pas les jeux en « Coopération ».
- Collections intelligentes : une règle « Genre » trouve aussi les modes de jeu (« Multijoueur », « Coopération »…). L'éditeur propose les genres et modes présents dans la bibliothèque au lieu d'un champ libre.

## [1.4.1] - 2026-11-09

### Ajouté
- Onglet **Goldberg** dans la fiche des jeux exécutables : installation, recherche de sauvegardes et options de l'émulateur.
- Options de Goldberg modifiables dans Ignite, par défaut pour tous les jeux et jeu par jeu : profil, overlay en jeu, connexion et DLC, compatibilité.
- Installeur aux couleurs d'Ignite (icône, image latérale, bandeau).

### Corrigé
- L'overlay en jeu ne s'affichait pas : Ignite configurait un DLL que les jeux Unity et Unreal ne chargent pas. Il trouve maintenant le bon DLL et remplace une build sans overlay par la build « experimental », en gardant une copie de l'ancien.

## [1.4.0] - 2026-11-09

### Ajouté
- Succès des jeux exécutables via l'émulateur Goldberg (gbe_fork) : installation, définitions générées depuis Steam, suivi en direct.
- Notifications de succès aux couleurs d'Ignite, avec l'icône du succès et un badge de rareté.
- Pourcentage de joueurs ayant débloqué chaque succès (statistiques publiques Steam) pour les jeux exécutables.

### Corrigé
- Ignite ne peut plus se figer à l'ouverture d'une notification ; le cache des succès reste à jour quand l'app est réduite.

## [1.3.0] - 2026-11-08

### Modifié
- L'application s'appelle désormais Ignite partout (identifiant `com.ignite.app`, base `ignite.db`). Les données de l'ancienne version sont déplacées automatiquement au premier lancement.

## [1.2.0] - 2026-11-08

### Ajouté
- Fiche de jeu à onglets : Aperçu, Succès, Captures, Actus, Lancement.
- Durée pour finir (IGDB) avec correction manuelle.
- Captures d'écran : Steam, Xbox Game Bar et dossiers personnels, avec visionneuse.
- Icônes des succès en cache, succès cachés et cadenas pour les succès verrouillés.

### Corrigé
- Battle.net : lecture des jeux installés et lancement via le client.

## [1.1.0] - 2026-11-06

### Ajouté
- Accueil « cinéma », mode Canapé à la manette, statistiques toutes sources.
- Récap de fin de session (note, note de reprise, statut).
- Sources : Steam, GOG Galaxy, Epic Games, Battle.net, EA app, Ubisoft Connect, Riot, exécutables, émulateurs (RetroAchievements).
- Suivi automatique du temps de jeu, collections intelligentes, recherche avancée, jaquettes SteamGridDB, bandes-annonces et actus.
- Icône dans la zone de notification avec le chrono du jeu en cours, démarrage avec Windows, installeur NSIS.

Les versions jusqu’à la 1.4.1 ont été construites avant la publication sur GitHub : elles n’ont pas de release associée.
