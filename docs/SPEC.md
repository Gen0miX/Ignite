# Spécification produit — Ludothèque

## 1. Vision

Steam est puissant mais sa bibliothèque est dense et ne connaît que les jeux Steam. Ludothèque est un launcher **plus lisible et plus utile au quotidien** :

- **une seule bibliothèque** pour tous les jeux du PC, quelle que soit leur origine ;
- un **lancement en un clic**, avec des profils de lancement ;
- le **même suivi pour tous les jeux** : temps de jeu, sessions, succès (ou objectifs perso quand la plateforme n'en a pas), statut ;
- des **filtres et collections intelligentes** pour retrouver vite un jeu ;
- des **statistiques toutes sources confondues**.

Utilisateur cible : un joueur PC (Windows) avec une grosse bibliothèque Steam (~130 jeux) et des jeux répartis sur d'autres launchers.

## 2. Concepts métier

| Concept | Définition |
|---|---|
| **Jeu** (`Game`) | Une entrée unique dans la bibliothèque, même si le jeu est possédé sur plusieurs plateformes. |
| **Source** (`GameSource`) | Un exemplaire du jeu sur une plateforme : Steam, Epic, GOG, Battle.net, EA, Ubisoft, Xbox, Riot, exécutable local, émulateur. Un jeu a 1..n sources. |
| **Session** | Une période de jeu : début, fin, durée, origine (`auto` = processus détecté, `manual`, `imported` = venant du launcher, `estimate` = saisie « temps déjà joué »). |
| **Succès** | Fourni par la plateforme (Steam, GOG, Xbox, RetroAchievements). |
| **Objectif perso** | Une case à cocher définie par l'utilisateur. Remplace les succès quand la source n'en fournit pas, et peut aussi les compléter. |
| **Statut** | `todo` (à faire), `playing` (en cours), `done` (terminé), `dropped` (abandonné). Choisi par l'utilisateur, avec une suggestion automatique (jamais lancé → `todo`). |
| **Collection** | Manuelle (liste choisie à la main) ou **intelligente** (définie par des règles, ex. `tag = souls-like`, `durée de session < 1 h`, `pas lancé depuis 1 an`). |
| **Profil de lancement** | Arguments, variables d'environnement, commandes avant et après le lancement, choix « via le launcher » ou « exécutable direct ». |

### Doublons multi-plateformes

Quand un jeu importé correspond à un jeu existant (même titre normalisé, même ID IGDB ou même ID SteamGridDB), on propose de **fusionner** : une seule fiche, plusieurs sources. L'utilisateur peut défusionner. Le bouton Jouer utilise la source préférée (réglable par jeu).

## 3. Écrans (maquettes : `design/mockups/`, détails dans `docs/DESIGN.md`)

### 3.1 Bibliothèque unifiée — `Multi-Bibliotheque.dc.html`

Mise en page en 3 colonnes :

- **Barre latérale :**
  - navigation : Tous / Installés / Favoris / Statistiques ;
  - **Sources**, avec le nombre de jeux par source ;
  - **Statuts**, avec leurs compteurs ;
  - collections intelligentes ;
  - « Gérer les sources ».
- **Zone centrale :**
  - titre et compteur ;
  - recherche avec syntaxe (`source:gog`, `tag:souls`, `heures:>20`, `statut:done`, texte libre) ;
  - bouton **Ajouter un jeu** ;
  - filtres en pastilles (source, installé, genre, durée de session, joueurs, Steam Deck, doublons) et tri (dernière session, nom, temps de jeu, date d'ajout, % de succès) ;
  - choix grille ou liste ;
  - grille de jaquettes 2:3 avec un badge de source et le temps / la progression des succès en dessous.
- **Panneau du jeu sélectionné :**
  - bannière ;
  - sources possédées ;
  - **Jouer**, avec le choix de la source ou du profil ;
  - temps total, succès, dernière session ;
  - répartition du temps par source ;
  - statut (contrôle segmenté) ;
  - note perso ;
  - exécutable, mode de suivi, jaquette ;
  - tags.

Interactions :

- Simple clic : sélectionne le jeu et ouvre le panneau.
- Double-clic ou Entrée : lance le jeu.
- Clic droit : menu contextuel (Jouer, Ouvrir la fiche, Changer le statut, Ajouter à une collection, Ouvrir le dossier, Masquer).
- Le filtre en cours se reflète dans l'URL interne, ce qui permet d'y revenir.

### 3.2 Ajouter un jeu — `Multi-Ajouter.dc.html`

Fenêtre modale avec 4 options : **Depuis un launcher**, **Scanner ce PC**, **Exécutable** (.exe/.lnk/.url, glisser-déposer accepté), **Émulateur / ROMs**.

Formulaire pour un exécutable :

- fichier ;
- nom affiché (prérempli depuis les métadonnées de l'exe ou le nom du dossier) ;
- arguments ;
- processus à suivre (automatique, le fichier choisi, ou un autre processus) ;
- temps déjà joué, facultatif et enregistré comme `estimate` ;
- choix de la jaquette parmi les résultats SteamGridDB, ou une image importée ;
- case « ajouter aux collections intelligentes correspondantes ».

### 3.3 Import des jeux détectés — `Multi-Import.dc.html`

Parcours en 4 étapes : Sources → Jeux détectés → Jaquettes & infos → Terminé.

- **Colonne de gauche :** l'état de chaque source analysée (connectée, non installée, connexion requise).
- **Tableau des jeux détectés :** case à cocher, jeu, source, état (installé, taille ou chemin), temps importé, succès disponibles, remarque. Remarques possibles :
  - « Doublon Steam → fusionner » ;
  - « Probablement pas un jeu », décoché par défaut. Exemples : `UnityCrashHandler64`, les installeurs, les outils anti-triche.
- **Bas de page :** le nombre de jeux sélectionnés et de fusions, puis Continuer.

### 3.4 Fiche d'un jeu — `Multi-Fiche.dc.html` (jeu non-Steam) et `Fiche.dc.html` (version Steam, ancien style)

- **En-tête :** bannière, badges de source et de genre, titre, Jouer, profil de lancement.
- **Chiffres clés :**
  - temps total, avec la part estimée ;
  - nombre de sessions et durée moyenne ;
  - succès ou objectifs ;
  - statut.
- **Temps de jeu sur 30 jours :** une barre par jour.
- **Journal des sessions :** date, horaire, durée, origine. Chaque session est modifiable et on peut ajouter une session manuelle.
- **Objectifs perso :** liste à cocher qu'on peut compléter.
- **Lancement :** fichier, arguments, processus suivi, commandes avant et après, overlay.
- **Métadonnées :** IGDB, SteamGridDB, durée HowLongToBeat.
- **Pour un jeu Steam, en plus :** les succès les plus proches, les amis qui y jouent, et les actus filtrées (les pubs pour d'autres jeux sont masquées).

### 3.5 Statistiques — `Multi-Stats.dc.html`

- **Période :** semaine, mois, année ou depuis toujours, avec un filtre par source.
- **Chiffres clés :** temps joué, jeux lancés, jeux terminés, succès débloqués, plus longue série de jours.
- **Calendrier annuel :** intensité par jour, sur 4 niveaux.
- **Jeux les plus joués :** barres colorées par source.
- **Répartition du temps par source :** barre empilée et légende.
- **État de la bibliothèque :** terminés, en cours, abandonnés, jamais finis.

### 3.6 Réglages › Sources & suivi — `Multi-Sources.dc.html`

- **Une carte par source :** état de connexion, interrupteur, ce que la source fournit (bibliothèque, temps, succès, amis), méthode de connexion et action (Modifier, Resynchroniser, Se connecter, Configurer).
- **Règles de suivi :**
  - méthode : détection de processus ;
  - session minimale (2 min par défaut) ;
  - pause si inactif (15 min par défaut) ;
  - jeux sur plusieurs plateformes : fusionner ;
  - import de l'historique des launchers : au premier import.

### 3.7 Plus tard (maquettes de la ligne « directions » sur le canvas)

- **Accueil** façon « Cinéma » (`Main.dc.html`) :
  - Reprendre le dernier jeu ;
  - « Ce soir, j'ai… » : suggestions selon le temps disponible ;
  - « Ta pile à finir » ;
  - actus filtrées ;
  - résumé de la semaine.
- **Mode Canapé** (`Canape.dc.html`) : navigation à la manette, plein écran, roue du hasard.

## 4. Règles métier

- **Temps total d'un jeu** = la somme, pour chacune de ses sources, du temps de cette source (règle détaillée dans `ARCHITECTURE.md` § Cumul).
- **Session minimale** : les sessions plus courtes que le seuil sont ignorées (crash, lancement par erreur).
- **Inactivité** : au-delà du seuil sans entrée clavier, souris ou manette, le chrono se met en pause. Le temps d'inactivité est retiré de la session.
- **Suggestion de statut** : 0 minute jouée → `todo`. Une session dans les 30 derniers jours → suggestion `playing` (sans l'imposer). 100 % des succès → suggestion `done`.
- **Actus** : on n'affiche que les actus des jeux lancés ces 30 derniers jours. Les annonces qui promeuvent un autre jeu sont masquées, avec un compteur « N actus masquées » qu'on peut déplier.
- **Faux jeux** : un exécutable détecté par le scan est écarté par défaut s'il correspond à une liste de motifs (`*crash*`, `*unins*`, `*setup*`, `*redist*`, `*launcher*` quand un autre exe existe dans le même dossier, `EasyAntiCheat*`, `BEService*`…).

## 5. Hors périmètre (pour l'instant)

macOS/Linux, achats en jeu, magasin intégré, chat entre amis, synchronisation cloud des sauvegardes de jeu, Steam Deck.
