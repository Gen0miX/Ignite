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

> Implémenté dans `src-tauri/migrations/0001_initial.sql`, avec en plus : clés étrangères `ON DELETE CASCADE` sur `game_tags` et `collection_games`, contraintes `CHECK` sur les énumérations (`status`, `origin`, `mode`, `kind`), index sur `game_sources(game_id)`. Le champ de règle `added_at` (§ 6) correspond à `games.created_at`.

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
2. **Pendant la session** : un heartbeat toutes les 60 s écrit `ended_at` et la durée provisoire en base, pour survivre à un crash ou à une coupure. Rien n'est écrit tant que la session n'a pas atteint le minimum (2 min par défaut) : un jeu fermé avant, ou un crash pendant ce délai, ne laisse aucune ligne. Au démarrage, les sessions `auto` sous le minimum (anciennes versions, app tuée par une mise à jour) sont supprimées. Une sonde d'inactivité (`GetLastInputInfo` pour le clavier et la souris, XInput pour la manette) accumule `idle_seconds` au-delà du seuil (15 min par défaut).
3. **Fin de la session** : tous les processus correspondants ont disparu depuis plus de 10 s (délai de grâce pour les launchers qui relancent un processus enfant). On enregistre `duration_seconds = durée totale − idle_seconds`. On supprime la session si elle est sous le minimum (2 min par défaut).
4. Les événements `session-started` et `session-ended` sont émis vers le frontend (badge « En jeu », mise à jour en direct).
5. **Récap de fin de session** (`docs/SESSION-RECAP.md`) : chaque pause retirée est aussi enregistrée (`session_idle_periods`, migration 0007) pour la frise. Pour une session `auto` gardée (pas à la fermeture de l'app), le tracker émet `session-recap-ready`, resynchronise les succès du jeu en arrière-plan (Steam, RetroAchievements ; GOG lit son cache), puis émet `session-recap-updated`. Le frontend écoute ces événements au-dessus du routeur (donc aussi en mode Canapé) : selon Réglages › Général › Fin de session (stocké dans le WebView, `ignite.prefs`), il ramène la fenêtre au premier plan (`window_reveal`) et ouvre la modale, ou, en mode Canapé ou si un autre jeu tourne, envoie une notification Windows (`notify`, plugin `tauri-plugin-notification`) et garde le récap en attente jusqu'au retour dans la fenêtre principale (le plugin ne remonte pas le clic sur une notification sous Windows). Les récaps s'affichent un par un. Commandes : `session_recap`, `session_recap_save` (note de reprise dans `games.resume_note` et copiée dans `play_sessions.note`, note 1–5 dans `play_sessions.rating`, statut), `session_discard`.

Implémentation (J2) :

- polling `sysinfo` toutes les 3 s (pas de WMI : `Win32_ProcessStartTrace` demande les droits administrateur) ; la liste des jeux surveillés et les réglages sont relus toutes les 30 s ;
- `process_matchers` au format `[{"exe_path": …} | {"path_prefix": …} | {"exe": …}]` ; par défaut, préfixe = dossier d'installation (Steam : `steamapps\common\<installdir>`). Un processus dont le chemin est illisible (processus élevé) ne peut correspondre qu'à un matcher `exe` ;
- **inactivité** : quand la durée sans entrée dépasse le seuil, c'est **toute la période d'inactivité** qui est retirée (pas seulement ce qui dépasse le seuil). Manette : XInput (`dwPacketNumber`), car `GetLastInputInfo` ne voit pas les manettes ;
- à la fermeture de l'app, les sessions ouvertes sont écrites ; en cas de crash, la dernière écriture (heartbeat) fait foi ;
- les sessions saisies à la main sur un jeu à source unique sont rattachées à cette source. Pour une source qui a son propre compteur (Steam), elles servent donc à l'historique, pas au total (règle de cumul ci-dessous) : l'écran le signale.

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

## 5 bis. Sources, import et doublons (J3)

- Chaque source a un interrupteur (`settings` : `sources.<kind>.enabled`). Steam est actif par défaut ; GOG et Epic restent inactifs tant que l'utilisateur ne les active pas (Réglages › Sources ou assistant d'import) : rien n'entre dans la bibliothèque sans son accord.
- Assistant d'import : `import_scan` lit les sources **sans rien écrire** et classe chaque jeu (nouveau / déjà importé / doublon d'un jeu d'une autre plateforme / probablement pas un jeu) ; `import_commit` enregistre les choix. Les jeux décochés sont mémorisés (`import.ignored`, clés `kind:external_id`) et filtrés des synchronisations suivantes.
- Doublons : clé de comparaison = titre sans casse, accents, ™/®, ponctuation ni mots d'édition (GOTY, Remastered, Enhanced Edition…). Deux jeux de la même plateforme ne sont jamais proposés à la fusion. La fusion déplace sources, sessions, objectifs, tags, collections et images manquantes ; la défusion détache une source et ses sessions.
- Images SteamGridDB : seules les URL `*.steamgriddb.com` sont téléchargées ; les vignettes du sélecteur sont elles aussi copiées dans `artwork/_sgdb/` avant affichage (pas de hotlinking).

## 5 ter. Finitions (J5)

- **Scanner ce PC** (`sources/folder_scan.rs`) : chaque sous-dossier d'un dossier choisi est un jeu candidat ; l'exe principal est deviné, les outils (crash handlers, installeurs, anti-triche, redistribuables) sont écartés.
- **Accueil** (`library/home.rs`) : « Reprendre », récents, « Ce soir, j'ai… » (score selon le temps disponible, la durée moyenne des sessions, le statut et la fraîcheur), pile à finir, actus Steam (`metadata/news.rs`, cache 30 min, promotions masquées), semaine.
- **Mode Canapé** (`/couch`, hors de la coque, `library/couch.rs`) : rayons (récents, favoris, coop, sessions courtes, tout) et roue du hasard par humeur (chill / intense selon les genres, coop selon les modes, « jamais lancé ») tirée dans la pile installée, ni terminée ni abandonnée. Manette via l'API Gamepad (mapping standard, polling `requestAnimationFrame`, répétition des directions ; logique pure dans `src/lib/gamepad.ts`), avec équivalents clavier. Plein écran via `setFullscreen` (permission `core:window:allow-set-fullscreen`). Le bouton Menu de la manette ouvre le mode Canapé depuis n'importe quel écran.
- **Zone de notification** (`tray.rs`) : infobulle et première ligne du menu = jeu en cours et son minuteur (rafraîchis toutes les 15 s) ; menu Afficher / Mode Canapé / Quitter. Fermer la fenêtre la cache (réglage `app.close_to_tray`, actif par défaut) pour que le suivi continue.
- **Démarrer avec Windows** : `tauri-plugin-autostart` (clé `Run` du registre) avec `--minimized` ; la fenêtre est créée invisible et ne s'affiche pas dans ce cas.
- **Installeur** : NSIS, installation par utilisateur (sans droits administrateur), en français.

## 5 quater. Navigation et actions sur les jeux

- **Menu latéral** : un rail d'icônes (Accueil, Bibliothèque, Statistiques, Canapé ; Importer, Réglages en bas ; minuteur du jeu en cours) visible partout, et un panneau Bibliothèque (vues, collections, statuts, sources, sections repliables mémorisées dans `localStorage`) affiché seulement sur `/library` et `/game/:id`. Le bouton en bas du rail réduit le menu aux icônes (libellés en infobulle ; le panneau Bibliothèque reste affiché), mémorisé dans `localStorage` (`ignite.menu`). Fiche du jeu : bande-annonce, images et description du Steam Store, et actus du jeu (commandes `game_store_media` et `game_news`). Les URL viennent toujours du backend, qui ne garde que celles en `https://*.steamstatic.com`. Le frontend charge ensuite ces images et le flux vidéo HLS directement depuis ce CDN : c'est l'exception de la CSP (`img-src`, `media-src`, `connect-src`), sans clé ni appel d'API. Les icônes de succès du récap viennent de `steamcdn-a.akamaihd.net`, `images.gog.com` et `media.retroachievements.org` (`img-src`). Quand IGDB complète, les images viennent de `images.igdb.com` et les vidéos du lecteur `youtube-nocookie.com` (`frame-src`), avec les miniatures de `i.ytimg.com`. Quand la largeur de la grille change (fiche ouverte ou fermée, menu réduit, fenêtre redimensionnée), les jaquettes glissent vers leur nouvelle place (FLIP, `src/lib/useGridFlip.ts`, désactivé si « réduire les animations ») et la fiche entre en glissant.
- **Jeux masqués** : absents de la bibliothèque, de l'accueil et des stats ; on les retrouve dans Réglages › Jeux masqués (réafficher un par un ou tous, ou la vue `/library?view=hidden`).
- **Retour visuel** : après une action (sélection multiple, menu d'une carte), une notification brève (`Toaster`, `aria-live`) confirme le changement, les cartes modifiées s'illuminent un instant et la sélection est vidée.
- **Titre des fiches** : le logo du jeu remplace son nom sur la fiche et dans le panneau quand il existe (texte alternatif = titre, retour au texte si l'image ne charge pas).
- **Actions sur une carte** : bouton « ⋯ », clic droit ou Maj+F10 (jouer, fiche, images, collection, favori, masquer, supprimer). Le panneau du jeu reprend ces actions sous le bouton Jouer.
- **Sélection multiple** : case sur la carte, Ctrl/Maj+clic, Espace ; barre d'actions (statut, masquer, collection, supprimer, tout sélectionner), Échap pour annuler. Commandes `games_bulk_update`, `collection_add_games`.
- **Suppression** (`games_delete`) : seulement les jeux dont toutes les sources sont des exécutables ajoutés à la main ; les jeux des launchers reviendraient à la synchro, on les masque. Sessions, objectifs et images en cache partent avec le jeu ; les fichiers du disque ne sont pas touchés.
- **Collections manuelles** : sélecteur commun (cocher/décocher, créer à la volée) et glisser-déposer des cartes sur une collection du panneau. Le glisser utilise les événements pointeur, pas le drag-and-drop HTML5 : sous Windows, Tauri réserve le dépôt de fichiers natif (utilisé par « Ajouter un jeu »), ce qui désactive le DnD HTML5 dans la page.

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

### Implémentation J4

- Un seul modèle de règles (`library/rules.rs`) sert aux collections intelligentes et à la recherche ; la barre de recherche est traduite en règles par `library/search.rs` (alias français : `statut:termine`, `heures:>20`, `joueurs:coop`, `joue:<30j`, `-` pour exclure, guillemets). Les pastilles de filtre modifient le texte de recherche lui-même. Champ supplémentaire : `mode` (joueurs). Un jeu jamais lancé compte comme « pas lancé depuis N jours ».
- Statistiques par période (`library/stats.rs`) : temps par jour = sessions datées + croissance des compteurs Steam/GOG **non expliquée** par ces sessions (calcul cumulé, donc aucun double comptage). Les compteurs sont relevés une fois par jour (`playtime_snapshots`) ; l'historique par période commence au premier relevé. « Depuis toujours » reprend la règle de cumul de la bibliothèque.
- `games.completed_at` est posé quand le statut passe à « terminé » (jeux terminés par période).
- Genres et modes de jeu : tags automatiques (`origin` = `genre` / `mode`, noms préfixés `genre:` / `mode:`), séparés des tags de l'utilisateur.

## 7. Métadonnées & images (`metadata/`)

- **Steam** : les images locales de `Steam\appcache\librarycache\<appid>\` (`library_600x900.jpg`, `library_hero.jpg`, `logo.png`, `header.jpg`, parfois dans un sous-dossier haché) viennent en premier. Sinon, le CDN Steam (`https://cdn.cloudflare.steamstatic.com/steam/apps/<appid>/library_600x900.jpg`, à vérifier).
- **Jeux non-Steam** : SteamGridDB (jaquettes 600×900, héros, logos), cherché par titre.
- **Genres et modes de jeu** (J4) : boutique Steam (`appdetails`, sans clé) pour les jeux Steam, base GOG Galaxy pour les jeux GOG, noms ramenés à une liste française commune. IGDB reste prévu pour les autres jeux (exécutables, Epic) et les descriptions. Durées pour finir : IGDB `game_time_to_beats` (table `game_time_to_beat`, migration 0008, cache 30 jours) et correction manuelle, qui prime champ par champ ; HowLongToBeat a été écarté (voir `INTEGRATIONS.md`). `games.hltb_main_minutes` garde la meilleure durée « histoire » pour les filtres (`duree:`), les collections et l'accueil.
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

Événements émis : `session-started`, `session-ended`, `session-recap-ready`, `session-recap-updated`, `import-progress`, `source-sync-finished`.
