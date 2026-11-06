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
2. **Densité maîtrisée** : 6 colonnes de jaquettes à 1440 px. Grille `repeat(auto-fill, minmax(150px, 1fr))` avec un écart de 18 px en vertical et 14 px en horizontal.
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
| **GameCard** | Jaquette 2:3 avec rayon 8. Badge de source en bas à gauche : 10 px en 700, fond `--source-badge-bg`, texte `--source-*`. Dessous : le titre (13 px, 600, ellipsis) et une ligne mono 11 muted « 186 h · 90 % ». Sélection : `outline: 2px solid var(--color-accent); outline-offset: 3px`. Survol : léger zoom (scale 1.02) et un bouton Jouer qui apparaît au centre. |
| **CoverPlaceholder** | Quand la jaquette manque : fond teinté (dérivé d'un hash du titre, dans des tons sombres), titre en 18 px 700 en bas. |
| **GamePanel** | Largeur 340–380, fond `--color-panel`, bordure gauche `--color-line`. Contenu de haut en bas : bannière de 200 px, badges des sources, Jouer avec le choix de la source, 3 StatTiles, la répartition du temps (barre empilée de 8 px), le statut (contrôle segmenté), la note perso, les détails et les tags. |
| **StatTile** | Padding 10 à 16, rayon 8 à 10, fond `--color-card`. Valeur en mono 16–26, libellé 11–12 muted. |
| **SegmentedControl** | Fond `--color-card` avec 4 px de marge intérieure. Segment de 36 px, rayon 7. Sélectionné : fond `--color-active`, 600. |
| **Tag** | Pastille ronde 12 px, padding 4 10, fond `--color-card`, texte `--color-text-2`. « + tag » en pointillés. |
| **SourceBadge** | Logo 18 px puis nom, 11 px 700, padding 3 8, rayon 5, fond `--color-active`, texte `--source-*`. Sur une jaquette : logo seul de 18 px dans un carré de 24 px (nom en infobulle et pour les lecteurs d'écran). |
| **Table** | En-têtes en 11 px majuscules muted. Lignes de 44 px séparées par `--color-line`. Elle défile horizontalement dans un conteneur `overflow-x: auto`. |
| **Modal** | Largeur max 900, rayon 16, fond `--color-panel`, ombre `--shadow-modal`. La bibliothèque reste visible derrière, assombrie (overlay noir à 60 % avec blur). En-tête et pied séparés par `--color-line`. |
| **Barres / graphiques** | Barres de 3 à 6 px de rayon. Barre du jour en cours ou valeur mise en avant en accent, les autres en `--color-active`. Calendrier : cases de 14 px avec `--heat-0..3`. |

## États et retours

- **En jeu** : quand une session est active, la carte du jeu porte une pastille « En jeu » (fond accent) et un minuteur en direct apparaît en bas de la barre latérale.
- **Chargement** : squelettes aux mêmes dimensions que le contenu (jaquettes 2:3 en `--color-card`).
- **Vide** : un message court et une action (ex. « Aucun jeu GOG trouvé · Ouvrir les réglages de la source »).
- **Erreur de source** : badge `--color-warn` dans la barre latérale et message dans Réglages › Sources.
- **Focus clavier** : `box-shadow: var(--focus-ring)` sur tous les éléments interactifs. Dans la grille, on se déplace avec les flèches et on lance avec Entrée.

## Écrans → maquettes

| Écran | Maquette | Route |
|---|---|---|
| Bibliothèque unifiée | `Multi-Bibliotheque.dc.html` | `/library` |
| Ajouter un jeu (modale) | `Multi-Ajouter.dc.html` | `/library?add=exe` |
| Import | `Multi-Import.dc.html` | `/import` |
| Fiche jeu | `Multi-Fiche.dc.html` | `/game/:id` |
| Statistiques | `Multi-Stats.dc.html` | `/stats` |
| Réglages › Sources | `Multi-Sources.dc.html` | `/settings/sources` |
| (Plus tard) Accueil | `Main.dc.html` (direction A, à adapter au thème Braise) | `/` |
| (Plus tard) Mode Canapé | `Canape.dc.html` | `/couch` |
