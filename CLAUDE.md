# CLAUDE.md — Ignite (ex-Ludothèque)

Ignite (nom de code et dépôt : `ludotheque`) est un launcher de jeux PC pour Windows qui regroupe en une seule bibliothèque les jeux Steam et non-Steam (Epic, GOG, Battle.net, exécutables locaux, émulateurs…). Il lance les jeux, suit le temps de jeu, les succès et les objectifs, et affiche des statistiques toutes sources confondues.

Lis dans cet ordre avant de coder :

1. `docs/SPEC.md` — périmètre produit, écrans, règles métier
2. `docs/ARCHITECTURE.md` — stack, modules, modèle de données, suivi du temps
3. `docs/INTEGRATIONS.md` — détails par source (Steam, Epic, GOG…)
4. `docs/DESIGN.md` + `design/tokens.css` — thème « Braise », composants, écrans
5. `docs/ROADMAP.md` — jalons et tâches, à suivre dans l'ordre

## Stack (décidée)

- **Tauri 2** (backend Rust) + **React 19 + TypeScript + Vite** (frontend)
- **SQLite** via `rusqlite` (migrations embarquées dans `src-tauri/migrations/`, fichier `%APPDATA%\com.ignite.app\ignite.db`)
- État frontend : **TanStack Query** pour les données backend, **Zustand** pour l'état UI
- Style : CSS Modules + variables CSS de `design/tokens.css` (pas de Tailwind, pas de librairie de composants lourde)
- Icônes : `lucide-react` (trait 2 px, cohérent avec les maquettes)
- Tests : `cargo test` (Rust), **Vitest** + Testing Library (frontend)
- Plateforme cible : **Windows 10/11 x64** d'abord. Garde le code spécifique à Windows derrière `#[cfg(windows)]` pour pouvoir porter plus tard.

## Commandes

```bash
pnpm install
pnpm tauri dev        # lancer l'app en dev (régénère aussi src/api/bindings.ts)
pnpm test             # tests frontend (Vitest)
pnpm test:rust        # tests Rust (= cargo test --manifest-path src-tauri/Cargo.toml)
pnpm bindings         # régénérer les types TS des commandes Tauri sans lancer l'app
pnpm lint && pnpm typecheck
pnpm format           # Prettier ; côté Rust : cargo fmt / cargo clippy --all-targets -- -D warnings
pnpm tauri build      # build installeur (NSIS)
```

Test touchant le vrai Credential Manager (ignoré par défaut) :
`cargo test --manifest-path src-tauri/Cargo.toml secrets -- --ignored`

Versions épinglées à connaître : `specta`/`tauri-specta` `=2.0.0-rc.26` et `specta-typescript` `=0.0.13` (Tauri Specta encore en RC) ;
`rusqlite` 0.40 (contrainte de `rusqlite_migration` 2.6) ; TypeScript 6.0 (typescript-eslint ne supporte pas TS 7) ;
secrets via `keyring-core` + `windows-native-keyring-store` (recommandé par keyring 4 pour une app).

**Attention, base de données en dev** : une instance lancée par les outils de Claude (Bash/PowerShell de l'app Claude) voit son `%APPDATA%` **virtualisé** par Windows vers `%LOCALAPPDATA%\Packages\Claude_pzs8sxrjxfjjc\LocalCache\Roaming\com.ignite.app\`. Pour tester sur la vraie base de l'utilisateur, lance `pnpm tauri dev` depuis le panneau Terminal (non virtualisé), et lis la base réelle depuis ce même terminal. L'app installée n'est pas concernée.

Nom : l'app s'appelle **Ignite** (`productName`, textes UI, logo `design/logo/ignite.svg` → `pnpm tauri icon design/logo/ignite.svg`). L'identifiant est `com.ignite.app` (base `ignite.db`) : `src-tauri/src/legacy.rs` déplace au démarrage les données de l'ancien `com.ludotheque.app` / `ludotheque.db`. Le nom du crate (`ludotheque`) et le service du coffre (`ludotheque`) restent inchangés : renommer le service ferait perdre les clés enregistrées. De même, `bundle.publisher` reste `ludotheque` (clé de registre où l'installeur retrouve l'installation précédente).

(Mets à jour cette section si tu changes l'outillage.)

## Règles

- **Langue** : l'interface est en français. Code, noms de fichiers, commits et commentaires en anglais. Centralise les textes UI dans `src/i18n/fr.ts`.
- **Secrets** : la clé Steam Web API et les jetons (SteamGridDB, IGDB, RetroAchievements…) ne vont **jamais** dans le frontend, les logs ou la base SQLite en clair. Stocke-les dans le coffre de l'OS (`keyring` crate / Windows Credential Manager). Le frontend n'appelle jamais d'API tierce directement : tout passe par des commandes Tauri.
- **Lecture seule sur les autres launchers** : on lit leurs fichiers et bases (manifests, SQLite GOG…) sans jamais les modifier. Copie un fichier SQLite tiers avant de l'ouvrir s'il peut être verrouillé.
- **Temps de jeu** : ne jamais compter deux fois le même temps (voir la règle de cumul dans `docs/ARCHITECTURE.md`). Chaque session garde son origine (`auto`, `manual`, `imported`, `estimate`).
- **Robustesse** : une source qui échoue (launcher absent, fichier introuvable, API hors ligne) ne doit jamais bloquer les autres. Logge l'erreur et affiche son état dans Réglages › Sources.
- **Accessibilité** : vrais `<button>`/`<a>`/`<input>`, focus visible, cibles ≥ 36 px (≥ 44 px pour les actions principales), contraste ≥ 4.5:1. Navigation clavier complète dans la grille.
- **Commandes Tauri** : typées des deux côtés (`specta`/`tauri-specta` recommandé pour générer les types TS).
- Petites PR/commits par tâche de la ROADMAP, avec des tests pour la logique métier (parsing des sources, calcul du temps, filtres).
- Ne pas inventer d'API : si un format de fichier ou un endpoint marqué « à vérifier » dans `INTEGRATIONS.md` ne correspond pas à ce que tu observes, adapte et mets la doc à jour.
