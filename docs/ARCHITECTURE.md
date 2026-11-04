# Architecture — Ludothèque

## 1. Vue d'ensemble

```
┌──────────────────────── Frontend (React + TS, WebView Tauri) ────────────────────────┐
│ routes/  Library · GameDetail · AddGame · ImportWizard · Stats · Settings            │
│ components/  GameGrid · GameCard · Sidebar · FilterBar · GamePanel · SessionChart …  │
│ api/  wrappers typés des commandes Tauri (générés via tauri-specta)                  │
└───────────────────────────────▲──────────────────────────────────────────────────────┘
                                │ invoke() / events
┌───────────────────────────────┴──────────── Backend (Rust, src-tauri) ───────────────┐
│ commands/   library, games, launch, sessions, import, stats, settings, artwork       │
│ sources/    trait Source + steam, epic, gog, battlenet, ea, ubisoft, xbox, riot,     │
│             local_exe, emulator                                                      │
│ tracker/    surveillance des processus, inactivité, sessions                         │
│ launcher/   lancement exe / URI, profils, commandes avant et après                   │
│ metadata/   steamgriddb, igdb, hltb (optionnel), cache d'images                      │
│ db/         rusqlite, migrations, repositories                                        │
│ secrets/    keyring (Windows Credential Manager)                                     │
└──────────────────────────────────────────────────────────────────────────────────────┘
```

Arborescence cible :

```
ludotheque/
├─ CLAUDE.md
├─ docs/  design/
├─ src/                     # frontend
│  ├─ app/ routes/ components/ api/ hooks/ i18n/ styles/
│  └─ main.tsx
└─ src-tauri/
   ├─ src/ { main.rs, commands/, sources/, tracker/, launcher/, metadata/, db/, secrets/ }
   ├─ migrations/
   └─ tauri.conf.json
```

## 2. Trait `Source`

```rust
#[async_trait]
pub trait Source: Send + Sync {
    fn kind(&self) -> SourceKind;                       // Steam, Epic, Gog, …
    async fn status(&self) -> SourceStatus;             // Connected | NotInstalled | NeedsAuth | Error(String)
    async fn scan(&self) -> Result<Vec<DetectedGame>>;  // jeux possédés et/ou installés
    async fn playtime(&self, ext_id: &str) -> Result<Option<PlaytimeInfo>>; // temps mesuré par la source, s'il existe
    async fn achievements(&self, ext_id: &str) -> Result<Option<Vec<Achievement>>>;
    fn launch_target(&self, src: &GameSource) -> LaunchTarget;  // Exe{path,args,cwd} | Uri(String)
    fn process_hints(&self, src: &GameSource) -> Vec<ProcessMatcher>; // pour le tracker
}
```

`DetectedGame { external_id, title, installed, install_path, exe_path, size_bytes, playtime_minutes, last_played, has_achievements }`

Chaque source est **isolée** : une erreur dans une source est renvoyée comme `SourceStatus::Error` et n'empêche pas l'import des autres.

## 3. Modèle de données (SQLite)

```sql
CREATE TABLE games (
  id TEXT PRIMARY KEY,                 -- uuid
  title TEXT NOT NULL,
  sort_title TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'todo', -- todo|playing|done|dropped
  favorite INTEGER NOT NULL DEFAULT 0,
  hidden INTEGER NOT NULL DEFAULT 0,
  notes TEXT,
  preferred_source_id TEXT,
  igdb_id INTEGER, steamgriddb_id INTEGER,
  hltb_main_minutes INTEGER,
  created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);

CREATE TABLE game_sources (
  id TEXT PRIMARY KEY,
  game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
  kind TEXT NOT NULL,                  -- steam|epic|gog|battlenet|ea|ubisoft|xbox|riot|local|emulator
  external_id TEXT,                    -- appid Steam, AppName Epic, gameId GOG, chemin ROM…
  installed INTEGER NOT NULL DEFAULT 0,
  install_path TEXT, exe_path TEXT, size_bytes INTEGER,
  launch_uri TEXT,                     -- ex. steam://rungameid/374320
  process_matchers TEXT,               -- JSON: [{"exe":"javaw.exe"} | {"path_prefix":"D:\\Games\\X"}]
  source_playtime_minutes INTEGER,     -- temps fourni par la source (Steam, GOG…), NULL sinon
  source_last_played TEXT,
  last_synced_at TEXT,
  UNIQUE(kind, external_id)
);

CREATE TABLE launch_profiles (
  id TEXT PRIMARY KEY,
  game_source_id TEXT NOT NULL REFERENCES game_sources(id) ON DELETE CASCADE,
  name TEXT NOT NULL, is_default INTEGER NOT NULL DEFAULT 0,
  mode TEXT NOT NULL DEFAULT 'auto',   -- auto|uri|exe
  args TEXT, env TEXT /*JSON*/, working_dir TEXT,
  pre_command TEXT, post_command TEXT
);

CREATE TABLE play_sessions (
  id TEXT PRIMARY KEY,
  game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
  game_source_id TEXT REFERENCES game_sources(id) ON DELETE SET NULL,
  started_at TEXT, ended_at TEXT,
  duration_seconds INTEGER NOT NULL,   -- temps actif (inactivité retirée)
  idle_seconds INTEGER NOT NULL DEFAULT 0,
  origin TEXT NOT NULL,                -- auto|manual|imported|estimate
  note TEXT
);
CREATE INDEX idx_sessions_game ON play_sessions(game_id, started_at);

CREATE TABLE achievements (
  id TEXT PRIMARY KEY,
  game_source_id TEXT NOT NULL REFERENCES game_sources(id) ON DELETE CASCADE,
  api_name TEXT NOT NULL, name TEXT, description TEXT, icon_url TEXT,
  unlocked_at TEXT, global_percent REAL,
  UNIQUE(game_source_id, api_name)
);

CREATE TABLE goals (
  id TEXT PRIMARY KEY,
  game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
  title TEXT NOT NULL, position INTEGER NOT NULL, done_at TEXT
);

CREATE TABLE tags (id TEXT PRIMARY KEY, name TEXT UNIQUE NOT NULL, origin TEXT /*igdb|steam|user*/);
CREATE TABLE game_tags (game_id TEXT, tag_id TEXT, PRIMARY KEY(game_id, tag_id));

CREATE TABLE collections (
  id TEXT PRIMARY KEY, name TEXT NOT NULL,
  kind TEXT NOT NULL,                  -- manual|smart
  rules TEXT,                          -- JSON (voir § 6)
  position INTEGER NOT NULL
);
CREATE TABLE collection_games (collection_id TEXT, game_id TEXT, PRIMARY KEY(collection_id, game_id));

CREATE TABLE artwork (
  game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
  kind TEXT NOT NULL,                  -- cover(600x900)|hero|logo|icon
  local_path TEXT NOT NULL, provider TEXT, source_url TEXT,
  PRIMARY KEY(game_id, kind)
);

CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL); -- JSON
```

## 4. Suivi du temps de jeu (`tracker/`)

1. **Démarrage d'une session** :
   - quand l'app lance un jeu : on connaît le jeu. On attend que l'un de ses `process_matchers` apparaisse (60 s maximum, sinon le PID lancé fait foi) ;
   - jeu lancé hors de l'app : une **surveillance en arrière-plan** (polling toutes les 3 s avec `sysinfo`, ou événements WMI `Win32_ProcessStartTrace` si disponibles) compare les processus en cours aux `process_matchers` de tous les jeux installés. Correspondances possibles, par ordre de priorité :
     1. chemin exact de l'exe ;
     2. préfixe `install_path` ;
     3. nom d'exe déclaré.
2. **Pendant la session** : un heartbeat toutes les 60 s écrit `ended_at` et la durée provisoire en base, pour survivre à un crash ou à une coupure. Une sonde d'inactivité (`GetLastInputInfo` pour le clavier et la souris, XInput pour la manette) accumule `idle_seconds` au-delà du seuil (15 min par défaut).
3. **Fin de la session** : tous les processus correspondants ont disparu depuis plus de 10 s (délai de grâce pour les launchers qui relancent un processus enfant). On enregistre `duration_seconds = durée totale − idle_seconds`. On supprime la session si elle est sous le minimum (2 min par défaut).
4. Les événements `session-started` et `session-ended` sont émis vers le frontend (badge « En jeu », mise à jour en direct).

Cas particuliers :

- **Launchers intermédiaires** (Minecraft, Battle.net, EA) : on suit le processus du jeu, pas celui du launcher. On le choisit à l'ajout, avec une détection automatique du processus enfant le plus long.
- **Steam** : on peut aussi lire l'état « en jeu » depuis la clé de registre `HKCU\Software\Valve\Steam\RunningAppID` (à vérifier).

### Cumul (pas de double comptage)

Pour chaque `game_source` :

- si la source fournit son propre compteur (`source_playtime_minutes` non NULL, cas de Steam et GOG) → **temps de la source = ce compteur**. Les sessions `auto` détectées pour cette source servent seulement à l'historique et aux graphiques, pas au total ;
- sinon → **temps de la source = somme des sessions** `auto` + `manual` + `imported` + `estimate` liées à cette source.

**Temps du jeu = somme du temps de ses sources.** Le panneau « Origine du temps de jeu » affiche cette décomposition.

## 5. Lancement (`launcher/`)

- `LaunchTarget::Uri` : ouverture via `ShellExecute` ou `tauri-plugin-shell` (`steam://rungameid/<appid>`, `com.epicgames.launcher://apps/<AppName>?action=launch&silent=true`, …).
- `LaunchTarget::Exe` : `std::process::Command` avec arguments, `cwd` (par défaut le dossier de l'exe) et variables d'environnement. Le processus est détaché pour que fermer Ludothèque ne tue pas le jeu.
- `pre_command` / `post_command` : exécutées via `cmd /C` avec un délai maximum. Leur sortie va dans les logs.
- Un seul jeu « en cours » à la fois par défaut. Si un deuxième lancement est demandé, on demande confirmation.

## 6. Collections intelligentes — format des règles

```json
{ "all": [
  { "field": "tag", "op": "in", "value": ["souls-like"] },
  { "field": "avg_session_minutes", "op": "lt", "value": 60 },
  { "field": "last_played", "op": "older_than_days", "value": 365 },
  { "field": "source", "op": "eq", "value": "gog" },
  { "field": "status", "op": "in", "value": ["todo", "dropped"] }
]}
```

Champs : `title`, `tag`, `genre`, `source`, `installed`, `status`, `favorite`, `playtime_minutes`, `avg_session_minutes`, `last_played`, `achievement_percent`, `hltb_main_minutes`, `added_at`. Opérateurs : `eq`, `neq`, `in`, `lt`, `gt`, `between`, `older_than_days`, `newer_than_days`, `contains`. Groupes `all` / `any` imbriqués.

La **barre de recherche** reprend les mêmes champs : `source:gog heures:>20 tag:souls statut:todo texte`. Un seul parseur sert aux deux (recherche et règles), avec des tests unitaires.

## 7. Métadonnées & images (`metadata/`)

- **Steam** : les images locales de `Steam\appcache\librarycache\<appid>\` (`library_600x900.jpg`, `library_hero.jpg`, `logo.png`, `header.jpg`, parfois dans un sous-dossier haché) viennent en premier. Sinon, le CDN Steam (`https://cdn.cloudflare.steamstatic.com/steam/apps/<appid>/library_600x900.jpg`, à vérifier).
- **Jeux non-Steam** : SteamGridDB (jaquettes 600×900, héros, logos), cherché par titre.
- **Genres, tags, description** : IGDB. Durées pour finir : HowLongToBeat (pas d'API officielle, facultatif, derrière un réglage).
- **Cache** : toutes les images sont copiées dans `app-data/artwork/<game_id>/` et servies au frontend via le protocole `asset:` de Tauri. On ne fait pas de hotlinking.

## 8. Commandes Tauri (première liste)

```
library_list(filter: LibraryFilter, sort: Sort) -> Vec<GameSummary>
game_get(id) -> GameDetail
game_update(id, patch: GamePatch)               // statut, note, favori, préférences
game_merge(ids: Vec<String>) / game_split(source_id)
launch_game(game_id, profile_id: Option<String>)
sessions_list(game_id, range) / session_upsert(session) / session_delete(id)
goals_list(game_id) / goal_upsert / goal_toggle / goal_delete
import_scan(sources: Vec<SourceKind>) -> Vec<ImportCandidate>
import_commit(candidates: Vec<ImportDecision>)
add_executable(input: AddExeInput) -> GameDetail
artwork_search(game_id) -> Vec<ArtworkOption> / artwork_set(game_id, kind, option)
stats_overview(range, sources) -> StatsOverview
sources_status() -> Vec<SourceStatusCard>
settings_get / settings_set
secrets_set(kind, value) / secrets_clear(kind)   // le frontend ne relit jamais un secret
```

Événements émis : `session-started`, `session-ended`, `import-progress`, `source-sync-finished`.
