# Design — direction B « Atelier », thème « Braise »

Le design de référence est la **direction B** : une interface dense et utilitaire, en 3 colonnes, avec beaucoup de filtres et des chiffres en police monospace. Le thème retenu est **Braise** : un fond charbon chaud et un accent orange.

- Les valeurs exactes sont dans `design/tokens.css`. **N'utilise jamais une couleur en dur dans les composants** : toujours une variable CSS.
- Les maquettes HTML sont dans `design/mockups/` (voir le README du dossier). Les 6 écrans « Multi-* » y sont encore dans la palette **Lime** d'origine : remplace leurs couleurs avec la table de correspondance ci-dessous. Seul `Multi-Bibliotheque` sait déjà afficher Braise (palette sélectionnable).

## Table de correspondance (maquettes Lime → variables Braise)

| Couleur dans les maquettes | Variable |
|---|---|
| `#101214` (fond) | `--color-bg` |
| `#14171A` | `--color-panel` |
| `#1B1F23` | `--color-card` |
| `#171A1D` | `--color-field` |
| `#0F1113` | `--color-field-sunken` |
| `#1E2226`, `#262B30`, `#2C3238` | `--color-active` |
| `#22262A` | `--color-line` |
| `#2A2F34`, `#343A40` | `--color-border` |
| `#E8EAE6` | `--color-text` |
| `#B4BAB0` | `--color-text-2` |
| `#8B9196` | `--color-muted` |
| `#C6F24E` (accent lime) | `--color-accent` (et `--color-accent-ink` pour du texte) |
| `rgba(198,242,78,0.12)` | `--color-accent-soft` |
| texte `#101214` sur l'accent | `--color-on-accent` |
| `#5BD68A` (ok) | `--color-ok` |
| `#E0703A` (abandonné) | `--status-dropped` (devient rose-rouge `#E05A6A`) |

## Principes

1. **La jaquette d'abord** : les images des jeux apportent la couleur, l'interface reste sobre autour. L'orange est réservé aux actions principales (Jouer, Ajouter), à la sélection et aux données mises en avant.
2. **Densité maîtrisée** : 6 colonnes de jaquettes à 1440 px par défaut. Grille `repeat(auto-fill, minmax(var(--cover-min), 1fr))` avec un écart de 18 px en vertical et 14 px en horizontal. L'utilisateur règle la largeur minimale des jaquettes (110 à 280 px, 150 par défaut, pas de 10) avec le contrôle **CoverSize** à droite de la barre d'outils : boutons loupe −/+ de 36 px autour d'un curseur (piste 4 px accent jusqu'à la valeur, double-clic = taille par défaut), mémorisé dans `ignite.prefs` ; les jaquettes glissent vers leur nouvelle place.
3. **Chiffres en monospace** (`--font-mono`) : temps, compteurs, pourcentages, chemins de fichiers.
4. **Libellés de section** : 11 px, MAJUSCULES, `letter-spacing: .1em`, graisse 600, `--color-muted`.
5. **Pas d'emoji**, pas de dégradés décoratifs. Les dégradés servent uniquement à rendre le texte lisible sur une image.
6. **Mise en page fluide** : barre latérale, contenu et panneau dans une ligne `flex-wrap`. Contenu en `flex: 999 1 560px; min-width: 0`. Sous ~1100 px, le panneau de droite passe en tiroir par-dessus le contenu.

## Composants

| Composant | Spécification |
|---|---|
| **Logo app** | Carré de 26 px, rayon 6, fond accent, triangle « play » en `--color-on-accent`, puis « Ludothèque » 17 px en 700. |
| **NavItem** | Hauteur 40 (36 pour les statuts), padding 0 10, rayon 8, texte 14. Compteur en mono 12 muted, aligné à droite. Actif : fond `--color-active`. |
| **SourceItem** | Pastille carrée de 22 px, rayon 6, fond `--source-*`, logo de la plateforme en 18 px, texte sombre, puis le nom et le compteur. Logos : [Simple Icons](https://simpleicons.org/) (CC0, paquet `simple-icons`) pour Steam, Epic, GOG, Battle.net, EA, Ubisoft et Riot ; Lucide pour Xbox (`Gamepad2`, retiré de Simple Icons à la demande de Microsoft), les exécutables (`AppWindow`) et les émulateurs (`Joystick`). Voir `SourceIcon.tsx`. |
| **StatusDot** | Pastille de 8 px aux couleurs `--status-*`. |
| **FilterChip** | Hauteur 36, padding 0 12, rayon 8, bordure `--color-border`, fond `--color-field`, 13 px en 500. Actif : bordure et texte `--color-accent-ink`, fond `--color-accent-soft`. Le libellé porte la valeur choisie (« Genre : RPG, Action ▾ »). |
| **SearchField** | Hauteur 40, icône loupe, placeholder en mono 13 qui montre la syntaxe (`source:gog heures:>20`). |
| **PrimaryButton** | Fond accent, texte `--color-on-accent`, 700. Tailles 40, 48 (panneau) et 52 (fiche), rayon 8 à 10. |
| **SecondaryButton** | Fond `--color-card` ou `--color-field`, bordure `--color-border`, 500. |
| **GameCard** | Jaquette 2:3 avec rayon 8. Badge de source en bas à gauche : 10 px en 700, fond `--source-badge-bg`, texte `--source-*`. Dessous : le titre (13 px, 600, ellipsis) et une ligne mono 11 muted « 228 h · 42/42 » (temps, puis le compte exact de succès ou d'objectifs). La progression est aussi une barre de 3 px au bas de la jaquette (fond `--color-line`, remplissage `--color-accent-ink`, `--status-done` à 100 %). Un jeu **non installé** a sa jaquette atténuée (`grayscale(.55) brightness(.55)`, normale au survol) et une icône `CloudDownload` de 12 px à côté du badge de plateforme ; « Installé » n'est plus écrit. Sélection : `outline: 2px solid var(--color-accent); outline-offset: 3px`. Survol et focus : léger zoom (scale 1.02), un bouton Jouer rond de 36 px (fond accent) dans le **coin bas-droit** et le bouton « ⋯ » dans le coin haut-droit, à gauche de la pastille de statut. Le centre de la carte sélectionne toujours, il ne lance jamais. Un lancement depuis la grille (bouton Jouer, double-clic, Entrée) passe par un toast « Lancement de … » avec **Annuler** pendant 2,5 s ; le panneau et la fiche lancent directement. |
| **CoverPlaceholder** | Quand la jaquette manque : fond teinté (dérivé d'un hash du titre, dans des tons sombres), titre en 18 px 700 en bas. |
| **GamePanel** | Largeur 340–380, fond `--color-panel`, bordure gauche `--color-line`. Contenu de haut en bas : bannière de 200 px, badges des sources, Jouer avec le choix de la source, une rangée d'actions (Ouvrir la fiche, Images, Collections en 3 colonnes égales, puis un bouton « ⋯ » de 44 px qui ouvre le menu de la carte : masquer, supprimer…), 3 StatTiles (valeur compacte « 229 h », « il y a 4 j », qui passe à la ligne plutôt que d'être coupée ; détail en infobulle), la répartition du temps (barre empilée de 8 px), le statut (contrôle segmenté), la note perso, les détails et les tags. |
| **StatTile** | Padding 10 à 16, rayon 8 à 10, fond `--color-card`. Valeur en mono 16–26, libellé 11–12 muted. |
| **SegmentedControl** | Fond `--color-card` avec 4 px de marge intérieure. Segment de 36 px, rayon 7. Sélectionné : fond `--color-active`, 600. |
| **Tag** | Pastille ronde 12 px, padding 4 10, fond `--color-card`, texte `--color-text-2`. « + tag » en pointillés. |
| **SourceBadge** | Logo 18 px puis nom, 11 px 700, padding 3 8, rayon 5, fond `--color-active`, texte `--source-*`. Sur une jaquette : logo seul de 18 px dans un carré de 24 px (nom en infobulle et pour les lecteurs d'écran). |
| **Table** | En-têtes en 11 px majuscules muted. Lignes de 44 px séparées par `--color-line`. Elle défile horizontalement dans un conteneur `overflow-x: auto`. |
| **Modal** | Largeur max 900, rayon 16, fond `--color-panel`, ombre `--shadow-modal`. La bibliothèque reste visible derrière, assombrie (overlay noir à 60 % avec blur). En-tête et pied séparés par `--color-line`. |
| **FlameRating** | Note de session en lecture seule : 5 flammes Lucide de 13 px, allumées en `--color-accent`, éteintes en `--color-border`, nom accessible « 4 flammes sur 5 ». Dans le journal des sessions (colonne Ressenti) et, en moyenne, sous le nombre de sessions de la fiche. Rien n'est affiché pour une session non notée. |
| **Accueil (Cinéma)** | Plus aéré que la bibliothèque : blocs espacés de 40 px, titres de section 22 px 700, rayons 12 à 24 px. Héros « Reprendre » de `min(540px, 62vh)` (360 px minimum), rayon 24 : image `hero` (repli `header`, puis jaquette), double dégradé de lisibilité, mention accent 13 px majuscules avec un point, logo du jeu (`max-height` 130, `max-width` 420 ; titre en texte s'il manque), note de reprise en 16 px `--color-text-2`, 3 chiffres (22 px 700, libellé muted dessous), boutons de 56 px : Jouer (accent, rayon 14), Fiche du jeu et Options de lancement (fond `rgb(255 255 255 / 8%)`, bordure à 22 %, menu « Jouer via … » seulement si plusieurs sources installées). « Ensuite » en bas à droite : 3 vignettes 150 × 84 qui changent le jeu du héros sans le lancer (masqué sous 1100 px). Rangées de jaquettes 2:3 rayon 12 (6 récents, 8 dans la pile avec étiquette pill en haut à gauche), 6 → 4 → 3 colonnes selon la largeur. Cartes latérales (« Ce soir, j'ai… », « Ta semaine ») : fond `--color-panel`, bordure `--color-line`, rayon 20, padding 24 ; pastilles de durée de 40 px (active en `--color-accent-soft` + bordure accent), 3 suggestions (jaquette 44 × 66, raison, bouton Jouer 44 px), « Surprends-moi » en pointillés. Actus : 3 cartes (image 120 px, type et date en accent), la 3ᵉ en pointillés compte les actus masquées. Sous 1100 px, les blocs côte à côte s'empilent. |
| **Fiche en onglets** | Sur **Aperçu** : héros complet et rangée de chiffres. Sur les autres onglets, le héros se replie en un **bandeau de 96 px** (fond, logo ou titre 22 px, « 145 h · En cours » en mono, Jouer de 48 px à droite). Barre d'onglets sous l'en-tête : des **liens** (`aria-current="page"`, ←/→ passent à l'onglet voisin), 44 px de haut, 14 px 600, muted ; l'onglet courant en `--color-text` avec un trait de 2 px accent ; compteur en mono 12 (« Succès 17/44 »). L'onglet Succès s'appelle « Objectifs » pour un jeu sans succès de plateforme, et n'apparaît pas s'il n'y a ni l'un ni l'autre. Aperçu : note de reprise (carte avec bordure gauche accent), temps de jeu sur 12 semaines (semaine en cours en accent), 5 dernières sessions et « Tout le journal », médias ; à droite les objectifs et un résumé du lancement. |
| **Durée de vie** | Carte de la colonne droite d'Aperçu (fond `--color-card`, rayon 12) : 3 barres de 8 px (`--color-accent` sur `--color-active`) à la même échelle (la plus longue = 100 %), libellé 12 px à gauche (112 px), valeur mono « ≈ 32 h » à droite. Repère vertical de 2 px en `--color-text` avec « Toi : 145 h » en mono 11 au-dessus, calé à droite avec une flèche au-delà de l'échelle. Phrase en 14 px `--color-text-2` (« Il te reste ≈ 12 h pour l'histoire », « Au-delà de la durée « histoire + extras » », « Tu as dépassé la durée complétionniste »). Pied séparé par `--color-line` : « Source : IGDB · d'après N joueurs · mis à jour … », Corriger, Voir sur IGDB ↗, Actualiser. « Corriger » : 3 champs en heures (valeurs IGDB en indication), candidats IGDB si la correspondance est douteuse. Tuile « Pour le finir » dans la rangée de chiffres : « ≈ 32 h » et « ≈ 12 h restantes » ou « histoire déjà dépassée ». |
| **Onglet Succès** | Résumé en 4 cartes (`--color-card`, rayon 12) : anneau `conic-gradient` de 88 px (accent sur `--color-active`) avec « x / y » en mono ; barre empilée par rareté (ultra-rares < 5 % en `--rarity-ultra`, rares 5–20 % en accent, courants en muted) avec légende ; déblocages par mois de la première à la dernière date (les 3 derniers mois en accent) ; ton plus rare. Barre d'outils : Tous / À débloquer / Débloqués avec les nombres (« À débloquer » par défaut s'il en reste), tri (plus faciles, plus rares, date, nom), case « Révéler les succès cachés », recherche. Liste en 2 colonnes (`minmax(360px, 1fr)`) : icône de 56 px depuis le cache d'images, bordure de la couleur de la rareté une fois débloqué ; pendant son téléchargement, la case de l'icône pulse comme un squelette (`img-pending`), puis l'image apparaît en fondu ; si le téléchargement échoue, une coupe ; verrouillé : cadenas à la place de l'image, barre de 4 px « 62 % des joueurs ». Succès caché verrouillé : « Succès caché », icône œil, bouton Révéler (un seul succès). Avec « Tous » : « Débloqués récemment » (4) puis les autres. |
| **Onglet Captures** | Pastilles de source (`aria-pressed`, accent doux quand active, muted à 0) avec leur nombre, puis « Ouvrir le dossier » et « Ajouter un dossier ». Groupes par session (« 29 oct. 2025 · session de 2 h 14 · 3 captures » ; une session après minuit reste un groupe), sinon par jour. Grille `auto-fill minmax(280px, 1fr)` de vignettes 16:9 (rayon 12, `content-visibility: auto`) : heure et résolution en mono, badge de source (Steam en `--source-steam`, Game Bar en `--source-xbox`). Au survol ou au focus, 4 boutons de 36 px en haut à droite (Agrandir, Utiliser comme fond, Copier, Explorateur). Vidéo : icône film. Moins de 4 captures : carte en pointillés « Plus de captures ? » avec les touches en `<kbd>`. Section « Images officielles » (fond, en-tête, captures de la boutique ; étiquette « Fond actuel »). |
| **Visionneuse de capture** | Plein écran dans la fenêtre (`rgb(8 6 5 / .97)`), image ou vidéo centrée, flèches rondes de 48 px désactivées aux extrémités, « 1 / n · ← → pour naviguer · Échap pour fermer » en mono. Panneau de 320 px (`--color-panel`) : logo, faits (date, session avec lien, source, taille et poids, fichier avec le chemin en `title`), « Utiliser comme fond du jeu » (principal, 44 px), Copier, Explorateur, app par défaut, champ Légende enregistré en quittant le champ ou avec Entrée. Le focus reste dans la visionneuse et revient à la vignette. Récap de session : ligne « n captures » avec 3 vignettes 16:9 et « Voir les captures ». |
| **SessionRecap** | Modale de fin de session (`components/SessionRecap`, maquette `Recap-Modale.dc.html`), largeur max 980. Bandeau de 200 px : héros avec double dégradé, mention « Session terminée · … » en accent 11 px majuscules, logo (ou titre), durée active en mono 52 px et comparaison à la session moyenne. Frise de 12 px (temps actif en `--color-accent`, pauses retirées en `--color-border`, libellés mono 11). Deux colonnes : 3 tuiles (Total avec « +2 h 14 » en `--color-ok`, Cette semaine, Série), succès débloqués en grille de 2 (icône 40 px, rareté en `--color-accent-ink` sous 15 %), objectifs à cocher ; puis « Où t'es-tu arrêté ? », 5 flammes (`radiogroup`, 44 px), statut (contrôle segmenté). Pied sur `--color-bg` : case « moins de N min », lien « Ne pas compter cette session », Ouvrir la fiche, Enregistrer (44 px). Échap ferme (confirmation si la note a changé), Ctrl+Entrée enregistre, focus dans la note et piégé dans la modale. |
| **Barres / graphiques** | Barres de 3 à 6 px de rayon. Barre du jour en cours ou valeur mise en avant en accent, les autres en `--color-active`. Calendrier : la grille occupe toute la largeur (case calculée entre 10 et 18 px, mois alignés sur les colonnes) avec `--heat-0..3`. Moins de 3 sessions sur la période : un encart « Ignite mesure tes sessions depuis le … » remplace le calendrier vide. |

## États et retours

- **En jeu** : quand une session est active, la carte du jeu porte une pastille « En jeu » (fond accent) et un minuteur en direct apparaît en bas de la barre latérale.
- **Chargement** : squelettes aux mêmes dimensions que le contenu (jaquettes 2:3 en `--color-card`). Les images (jaquettes, bannières, captures, vignettes) passent par `SkeletonImage` : invisibles jusqu'au chargement, leur conteneur (classe globale `img-host`) pulse entre `--color-active` et `--color-card`, puis l'image apparaît en fondu. Dans le sélecteur d'images (SteamGridDB), une grille de squelettes à la forme du type choisi (jaquette 2:3, fond 96:31, logo 2:1) s'affiche pendant la recherche, et chaque vignette garde sa case fixe pendant son chargement. Composant partagé `Skeleton` / `SkeletonGroup` (`components/Skeleton`) : blocs `--color-active` sur les cartes, pulsation douce (coupée si mouvement réduit), un seul message lu par les lecteurs d'écran (`role="status"`). Utilisé pour les succès, les actus et les médias de la fiche.
- **Vide** : un message court et une action (ex. « Aucun jeu GOG trouvé · Ouvrir les réglages de la source »).
- **Erreur de source** : badge `--color-warn` dans la barre latérale et message dans Réglages › Sources.
- **Focus clavier** : `box-shadow: var(--focus-ring)` sur tous les éléments interactifs. Dans la grille, on se déplace avec les flèches et on lance avec Entrée.

## Écrans → maquettes

| Écran | Maquette | Route |
|---|---|---|
| Bibliothèque unifiée | `Multi-Bibliotheque.dc.html` | `/library` |
| Ajouter un jeu (modale) | `Multi-Ajouter.dc.html` | `/library?add=exe` |
| Import | `Multi-Import.dc.html` | `/import` |
| Fiche jeu | `Multi-Fiche.dc.html`, puis `Fiche-Apercu`, `Fiche-Succes`, `Fiche-Captures`, `Fiche-Visionneuse` (`docs/FICHE-ONGLETS.md`) | `/game/:id/:tab?` |
| Statistiques | `Multi-Stats.dc.html` | `/stats` |
| Réglages | `Multi-Sources.dc.html` (cartes) ; catégories à gauche, une à la fois : Général, Sources, Émulateurs, Succès et suivi, Captures, Comptes et clés, Jeux masqués, À propos | `/settings/:section` (`general`, `sources`, `emulators`, `achievements`, `captures`, `accounts`, `hidden`, `about`) |
| Accueil « Cinéma » | `Main.dc.html` (direction A, sans son en-tête, thème Braise ; voir `docs/HOME-V2.md`) | `/` |
| (Plus tard) Mode Canapé | `Canape.dc.html` | `/couch` |
