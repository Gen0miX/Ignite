# Roadmap — jalons et tâches

Chaque tâche = une PR ou un commit, avec ses tests. Ne passe au jalon suivant que lorsque les critères « Fini quand » sont remplis.

## J0 — Socle (1 à 2 jours)

- [x] Créer le projet Tauri 2 + React + TS + Vite, avec pnpm, ESLint, Prettier, `cargo fmt` et clippy.
- [x] Intégrer `design/tokens.css`, les polices en local et un fichier `global.css` (reset, scrollbars sombres, focus ring).
- [x] Mettre en place la base SQLite (`rusqlite`) avec la migration initiale (schéma d'`ARCHITECTURE.md` § 3) et des repositories.
- [x] Générer les types TS des commandes Tauri (`tauri-specta`).
- [x] Créer le module `secrets` (keyring), avec des tests.
- [x] Construire la coquille de l'app : barre latérale et zone de contenu, routeur (`/library`, `/game/:id`, `/stats`, `/settings/sources`, `/import`), fichier `i18n/fr.ts`.

**Fini quand** : `pnpm tauri dev` ouvre la coquille vide au thème Braise, et la migration crée la base.

## J1 — Bibliothèque Steam (MVP lisible)

- [x] `sources/steam` en local : chemin Steam, `libraryfolders.vdf`, `appmanifest_*.acf` → jeux installés.
- [x] `sources/steam` via le Web API : `GetOwnedGames` (avec clé et SteamID saisis dans Réglages) → tous les jeux possédés et leur temps.
- [x] Images Steam : lecture de `appcache\librarycache` (avec la recherche dans les sous-dossiers hachés) et copie dans le cache de l'app.
- [x] Écran Bibliothèque : barre latérale (nav, sources, statuts), grille de GameCards, recherche texte, tri.
- [x] Panneau du jeu sélectionné : chiffres, statut, note perso, tags.
- [x] Lancement : `steam://rungameid/<appid>`.

**Fini quand** : la vraie bibliothèque Steam de l'utilisateur s'affiche avec ses jaquettes, on peut filtrer et trier, et Jouer lance le jeu.

## J2 — Suivi du temps & fiche jeu

- [x] `tracker` : surveillance des processus, heartbeat, délai de grâce, inactivité, session minimale (§ 4).
- [x] Événements `session-started` et `session-ended`, pastille « En jeu », minuteur en direct.
- [x] Règle de cumul sans double comptage (§ 4 Cumul), avec des tests unitaires.
- [x] Fiche jeu : temps sur 30 jours, journal des sessions (modifier, ajouter une session manuelle), objectifs perso, configuration de lancement.
- [x] Succès Steam : `GetPlayerAchievements` + `GetSchemaForGame`, mis en cache.

**Fini quand** : lancer et quitter un jeu crée une session correcte, et la fiche l'affiche.

## J3 — Jeux non-Steam

- [x] Modale « Ajouter un jeu » › Exécutable (fichier, nom, arguments, processus suivi, temps déjà joué).
- [x] SteamGridDB : recherche et choix de la jaquette, du héros et du logo.
- [x] `sources/epic` (manifests) et `sources/gog` (copie de `galaxy-2.0.db`, temps, succès).
- [x] Parcours d'import (4 étapes) : détection, doublons, faux jeux, import du temps.
- [x] Fusion et défusion des doublons, choix de la source préférée.
- [x] Badges de source et filtre Source dans la bibliothèque.

**Fini quand** : un jeu GOG possédé aussi sur Steam n'a qu'une fiche, avec les deux temps décomposés.

## J4 — Filtres avancés & statistiques

- [x] Parseur de recherche (`source:`, `tag:`, `heures:>`, `statut:`…), avec des tests.
- [x] Collections intelligentes : éditeur de règles et calcul.
- [x] Pastilles de filtre (genre via IGDB, durée de session, joueurs).
- [x] Écran Statistiques : chiffres clés, calendrier annuel, jeux les plus joués, répartition par source, état de la bibliothèque.
- [x] Réglages › Sources & suivi : cartes de source et règles du chrono.

## J5 — Sources secondaires & finitions

- [x] Battle.net, Ubisoft Connect, EA app, Riot (détection et lancement, suivi local).
- [x] Scan de dossiers et filtre des faux jeux.
- [x] Émulateurs + RetroAchievements.
- [x] Accueil « Cinéma » adapté au thème Braise, avec « Ce soir, j'ai… ».
- [x] Mode Canapé (manette).
- [x] Installeur, démarrage avec Windows (option), icône dans la zone de notification avec le minuteur.

## J6 — Fiche en onglets, durées de vie, succès et captures

Specs : `docs/FICHE-ONGLETS.md`, `docs/TIME-TO-BEAT.md`.

- [x] Fiche en onglets (`/game/:id/:tab`) : Aperçu, Succès (ou Objectifs), Actus, Lancement ; bandeau compact de 96 px hors Aperçu ; temps de jeu sur 12 semaines ; 5 dernières sessions avec le journal complet dépliable.
- [x] Durées de vie via IGDB (`game_time_to_beats`) : migration 0008, carte « Durée de vie », tuile « Pour le finir », correction manuelle, jeton `duree:`. HowLongToBeat écarté : le site refuse les clients qui ne se font pas passer pour un navigateur.
- [x] Onglet Succès : résumé, filtres, tri, succès cachés, icônes en cache.
- [x] Onglet Captures et visionneuse : Steam, Xbox Game Bar, dossiers perso, en lecture seule (migration 0010, miniatures dans `artwork/thumbs/`, ligne « n captures » dans le récap de session).

