# Kit de démarrage — Ludothèque (pour Claude Code)

## Contenu

```
CLAUDE.md                 ← instructions permanentes lues automatiquement par Claude Code
PROMPT.md                 ← premier message à coller dans Claude Code
docs/SPEC.md              ← produit : concepts, écrans, règles métier
docs/ARCHITECTURE.md      ← stack, modules, schéma SQLite, suivi du temps, commandes Tauri
docs/INTEGRATIONS.md      ← Steam, Epic, GOG, Battle.net… (fichiers, API, lancement)
docs/DESIGN.md            ← thème Braise, composants, correspondance avec les maquettes
docs/ROADMAP.md           ← jalons J0 → J5 avec critères de fin
design/tokens.css         ← variables CSS du thème Braise
design/mockups/           ← sources HTML des maquettes (référence de mise en page)
```

## Mode d'emploi

1. Crée un dossier vide pour le projet (ex. `C:\Dev\ludotheque`) et décompresse ce kit dedans.
2. Prérequis sur ton PC : Rust (déjà installé chez toi), Node 20+ et pnpm, les Build Tools Visual Studio (C++) et WebView2 (déjà présent sur Windows 11).
3. Ouvre Claude Code dans ce dossier et colle le contenu de `PROMPT.md`.
4. Prépare (pas besoin tout de suite, seulement au jalon J1/J3) :
   - une **clé Steam Web API** : https://steamcommunity.com/dev/apikey ;
   - ton **SteamID64** (visible sur ton profil, ou trouvé par l'app via `loginusers.vdf`) ;
   - une **clé API SteamGridDB** (compte gratuit, dans les préférences du compte) ;
   - en option, un **Client ID/Secret Twitch** pour IGDB.

   Tu les saisiras dans l'app (Réglages › Sources) : ne les donne pas en clair dans le chat ni dans le code.
5. Avance jalon par jalon (`docs/ROADMAP.md`). À la fin de chacun, teste sur ta vraie bibliothèque avant de continuer.

## Choix faits pour toi (modifiables)

- **Tauri 2 + Rust + React/TypeScript** plutôt qu'Electron : app légère, accès natif à Windows (processus, registre, fichiers des launchers), et tu as déjà Rust installé. Si tu préfères Electron + Node, dis-le à Claude Code dès le départ : le reste des docs reste valable.
- Le nom « Ludothèque » est provisoire.
