# Fiche jeu — onglets Aperçu, Succès, Captures

Maquettes : `design/mockups/Fiche-Apercu.dc.html`, `Fiche-Succes.dc.html`, `Fiche-Captures.dc.html`, `Fiche-Visionneuse.dc.html`.
Jeu d'exemple : ENDLESS Space 2. La capture utilisée est une vraie capture Steam de l'utilisateur. Les noms de succès entre crochets et les chiffres sont des exemples.

## Structure

- La fiche (`GameDetailPage`) garde son en-tête et sa rangée de chiffres, puis une **barre d'onglets** : Aperçu · Succès `[x]/[y]` · Captures `n` · Actus · Lancement.
  - Route : `/game/:id/:tab?` (`apercu` par défaut, puis `succes`, `captures`, `actus`, `lancement`). Le navigateur retient l'onglet.
  - Les onglets sont des liens (`<a aria-current="page">`), pas un `role="tablist"`. On les parcourt avec Tab, et les touches flèches gauche/droite passent à l'onglet voisin.
  - On ne montre l'onglet Succès que si le jeu a des succès (Steam, GOG, RetroAchievements, jeu local avec exécutable) ou des objectifs perso. Pour un jeu sans succès de plateforme, l'onglet s'appelle « Objectifs ».
- Sur un onglet autre qu'Aperçu, le héros se **replie** en un bandeau de 96 px : héros en fond, logo, « 145 h · En cours » et le bouton Jouer. Le héros complet ne s'affiche que sur Aperçu.
- **Aperçu** :
  - la note de reprise ;
  - le graphique « Temps de jeu · 12 dernières semaines » ;
  - les dernières sessions (avec la note en flammes du récap) ;
  - à droite : la carte « Durée de vie » (voir `TIME-TO-BEAT.md`), les objectifs, et un résumé du lancement ;
  - la bande-annonce et les images (`MediaSection`) restent sur Aperçu, sous les sessions, comme demandé dans `UI-FIXES.md` § 9.
- Les **Actus** et le **Lancement** passent dans leur propre onglet, sans changer leur contenu actuel.

## Onglet Succès

- **Résumé**, en 4 cartes sur une rangée :
  - un anneau de progression (`conic-gradient`) avec « [x] / [y] » et le pourcentage ;
  - tes succès par rareté : barre empilée Ultra-rares < 5 %, Rares 5–20 %, Courants, avec la légende et les nombres ;
  - les déblocages au fil du temps : barres par mois, de la première à la dernière date `unlocked_at`, les 3 derniers mois en accent ;
  - ton plus rare : son nom et son pourcentage.
- **Barre d'outils** :
  - contrôle segmenté Tous / À débloquer / Débloqués, avec les nombres. Par défaut : « À débloquer » s'il en reste, sinon « Tous » ;
  - tri : « les plus faciles d'abord » (pourcentage mondial décroissant), « les plus rares d'abord », « date de déblocage », « nom » ;
  - case « Révéler les succès cachés », désactivée par défaut ;
  - recherche dans les noms et descriptions.
- **Liste** en 2 colonnes : icône de 56 px (débloqué : icône Steam en couleur ; verrouillé : en niveaux de gris, avec l'opacité réduite), nom, description, puis :
  - pour un succès verrouillé : une barre de 4 px avec « 62 % des joueurs » ;
  - pour un succès débloqué : la date à droite, et la rareté en couleur (ultra-rare `#FFD0B8`, rare `--color-accent-ink`, sinon muted). La bordure de l'icône prend la couleur de la rareté.
- **Succès cachés** (Steam `hidden = 1`), quand ils sont verrouillés et que l'option est désactivée : titre « Succès caché », texte « Steam ne montre la description qu'une fois débloqué. », icône œil, et un bouton **Révéler** qui ne dévoile que ce succès-là.
- Sections : avec « À débloquer », les plus accessibles d'abord ; avec « Tous », d'abord « Débloqués récemment » (les 4 derniers), puis le reste.
- Données : la table `achievements` existe déjà (`unlocked_at`, `global_percent`). Ajoute `hidden INTEGER` et `icon_gray_url` si c'est nécessaire (`GetSchemaForGame` fournit `hidden`, `icon` et `icongray`). Les icônes vont dans le cache d'images, jamais en lien direct.
- **Jeu local** (`kind = 'local'` avec un exécutable) : le bloc « Succès du jeu (Goldberg) » remplace le message « pas de succès » (`docs/INTEGRATIONS.md`). Il montre l'état (DLL, dossier de configuration, sauvegardes, architecture), l'AppID Steam et les actions installer/réinstaller, vérifier, ouvrir les sauvegardes et oublier l'AppID. Les icônes des succès sont alors des fichiers locaux du dossier du jeu, servis par `asset:`.

## Onglet Captures

- **Sources** de captures, toutes en **lecture seule** (on ne déplace ni ne supprime jamais un fichier) :
  1. **Steam** : `Steam\userdata\<steamid3>\760\remote\<appid>\screenshots\*.jpg`, avec des miniatures dans `screenshots\thumbnails\`. Format **confirmé** sur la machine de l'utilisateur. Le nom de fichier `YYYYMMDDhhmmss_n.jpg` donne la date.
  2. **Xbox Game Bar** : `%USERPROFILE%\Videos\Captures\*.png|*.mp4`. Le nom contient le titre de la fenêtre du jeu. On fait la correspondance par titre normalisé, ou par exe quand on peut.
  3. **Dossiers perso** : l'utilisateur ajoute un dossier et l'associe à un jeu (ou à « tous les jeux », avec la même correspondance par nom de fichier).
- **Regroupement** :
  - par session : une capture dont la date tombe dans `[started_at, ended_at]` d'une session est rattachée à cette session (« 29 oct. 2025 · session de 2 h 14 · 3 captures ») ;
  - sinon, par jour.
- **Grille** : 3 colonnes en 16:9, en `auto-fill` (minimum 280 px).
  - En bas de chaque vignette : l'heure, la résolution et un badge de source.
  - Au survol ou au focus : Agrandir · Utiliser comme fond du jeu · Copier · Afficher dans l'Explorateur.
- **Filtres** en pastilles par source, avec leur nombre, plus « Ouvrir le dossier » et « + Ajouter un dossier ».
- **Peu de captures** (< 4) : une carte en pointillés « Plus de captures ? » rappelle les raccourcis (F12 dans Steam, Win+Alt+Impr pour la Game Bar).
- **Images officielles** : une section en dessous avec le héros, l'en-tête et les captures de la boutique déjà récupérées par `MediaSection`. Le fond actuel porte l'étiquette « Fond actuel ».
- **Visionneuse** (plein écran dans la fenêtre) :
  - l'image en grand, les flèches précédente/suivante (désactivées aux extrémités), « 1 / n · ← → pour naviguer · Échap pour fermer » ;
  - un panneau à droite de 320 px :
    - logo du jeu ;
    - date, session (lien vers le journal), source, taille et poids, nom de fichier (chemin complet dans `title`) ;
    - actions : **Utiliser comme fond du jeu** (bouton principal : copie l'image dans le cache d'images comme `hero` personnalisé, avec la même logique que « Changer les images »), Copier l'image, Afficher dans l'Explorateur, Ouvrir avec l'app par défaut ;
    - un champ **Légende**, enregistré dans Ignite.
- **Données** : on n'enregistre que des métadonnées. Nouvelle table `screenshots(id, game_id, source, path, taken_at, width, height, bytes, session_id, caption, thumb_path)`, et `screenshot_folders(path, game_id NULL)` pour les dossiers perso. Un scan au démarrage et à la fin de chaque session (`session-ended`), puis une surveillance de dossier (crate `notify`) pendant que l'app tourne.
- **Performances** : miniatures générées une seule fois dans `app-data/thumbs/` (taille max 480 px, crate `image`), chargement progressif, défilement virtualisé au-delà de 200 captures.
- **Récap de session** : si des captures ont été prises pendant la session, ajoute une ligne « 3 captures » avec 3 vignettes dans la modale de `SESSION-RECAP.md`.

## Tests

- Lecture de la date dans les noms de fichiers Steam et Game Bar.
- Rattachement d'une capture à une session (limites incluses, passage à minuit).
- Correspondance des titres Game Bar.
- « Utiliser comme fond » qui crée bien une illustration personnalisée sans toucher au fichier d'origine.
- Frontend : navigation clavier dans la visionneuse, retenue de l'onglet de la fiche, filtre par source.
