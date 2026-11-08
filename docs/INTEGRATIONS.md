# Intégrations par source

> Les chemins et formats ci-dessous sont des **points de départ**. Ceux qui sont marqués « à vérifier » doivent être confirmés sur une vraie machine avant d'écrire le parseur. Le projet open source **Playnite** (plugins de bibliothèques) est une bonne référence pour les détails de chaque launcher.

## Steam (priorité 1)

**Détection locale**

- Dossier Steam : registre `HKCU\Software\Valve\Steam\SteamPath`. Sur la machine de l'utilisateur, il est confirmé : `C:\Program Files (x86)\Steam`.
- Bibliothèques : `steamapps\libraryfolders.vdf` (format KeyValues/VDF texte, crate `keyvalues-parser` ou équivalent).
- Jeux installés : `<library>\steamapps\appmanifest_<appid>.acf` (nom, `installdir`, `SizeOnDisk`, `LastUpdated`).
- Images : `appcache\librarycache\<appid>\` contient `library_600x900.jpg`, `library_hero.jpg`, `library_hero_blur.jpg`, `logo.png` et `header.jpg`. **Confirmé** sur la machine de l'utilisateur. Attention : chez certains jeux, ces fichiers sont dans un **sous-dossier au nom haché**, donc il faut chercher récursivement sur 1 niveau.
- Temps local par jeu : `userdata\<steamid3>\config\localconfig.vdf` (champ `Playtime` sous `apps`, à vérifier). Sert de solution de repli sans clé API.

**Web API** (clé fournie par l'utilisateur, stockée dans le coffre de l'OS)

- `IPlayerService/GetOwnedGames/v1?key=&steamid=&include_appinfo=1&include_played_free_games=1` : la bibliothèque complète, avec `playtime_forever` (en minutes) et `rtime_last_played`.
- `IPlayerService/GetRecentlyPlayedGames/v1` : les jeux joués récemment.
- `ISteamUserStats/GetPlayerAchievements/v1?appid=&steamid=&l=french` : les succès débloqués.
- `ISteamUserStats/GetSchemaForGame/v2?appid=&l=french` : les noms et icônes des succès.
- `ISteamUserStats/GetGlobalAchievementPercentagesForApp/v2?gameid=` : la rareté des succès (`percent` peut être un nombre ou une chaîne selon les jeux : on accepte les deux).
- Succès (J2) : un jeu sans succès répond **HTTP 400** (`"Requested app has no stats"`), un profil privé **HTTP 403**. Le cache SQLite est valable 6 h ; `game_sources.achievements_total = 0` mémorise « pas de succès ». Les icônes (CDN Steam) ne sont pas encore affichées : pas de hotlinking, elles seront mises en cache plus tard.
- `ISteamUser/GetPlayerSummaries/v2` et `GetFriendList/v1` : les amis (pour plus tard).
- `ISteamNews/GetNewsForApp/v2` : les actus de chaque jeu (à filtrer, voir SPEC § 4).
- Boutique (sans clé, vérifié en J4) : `https://store.steampowered.com/api/appdetails?appids=<appid>&l=french&filters=genres,categories` → genres en français et catégories (id 2 Solo, 1/36/37/47/49 multijoueur, 9/38/48 coopération, 24/39/44 écran partagé). **Un seul appid par requête** avec ces filtres ; limite d'environ 200 requêtes / 5 min (HTTP 429 au-delà) : une requête toutes les 1,6 s en tâche de fond.
- Le SteamID64 vient de `config\loginusers.vdf`, ou bien l'utilisateur le saisit.
- Limiter les appels (~1 requête/s, cache 15 min). Le profil doit être public, ou la clé doit appartenir au compte.

**Lancement** : `steam://rungameid/<appid>`. **Processus à suivre** : les exe sous `<library>\steamapps\common\<installdir>\`.

### Vérifié sur la machine de l'utilisateur (J1, novembre 2026)

- `HKCU\Software\Valve\Steam\SteamPath` renvoie le chemin **en minuscules avec des `/`** (`c:/program files (x86)/steam`) : on le normalise et on préfère la casse de `HKLM\SOFTWARE\WOW6432Node\Valve\Steam\InstallPath`. `RunningAppID` existe bien (0 hors jeu).
- `libraryfolders.vdf` : une entrée par bibliothèque avec `path` (ici `C:\Program Files (x86)\Steam` et `E:\SteamLibrary`). Une bibliothèque sur un disque débranché est signalée en avertissement, pas en erreur.
- **Manifests en double** : un jeu déplacé peut laisser un ancien `appmanifest` dans une autre bibliothèque (ici Satisfactory, CS2, Total War: WARHAMMER III et Valheim, présents sur `C:` et `E:`). La bibliothèque qui fait foi est celle dont le bloc `"apps"` de `libraryfolders.vdf` contient l'appid ; à défaut, on garde le manifest entièrement installé puis le plus récent (`LastUpdated`). Sans ça, le suivi du temps surveille le mauvais dossier.
- `appmanifest_*.acf` : `appid`, `name`, `installdir`, `SizeOnDisk`, `LastPlayed` (Unix), `StateFlags` (bit 4 = entièrement installé). **Certains manifests peuvent être entièrement remplis d'octets nuls** (corruption côté Steam) : ils sont ignorés. Les outils Steam (Proton, Steam Linux Runtime, Steamworks Common Redistributables) sont exclus.
- `userdata\<accountId>\config\localconfig.vdf` : **confirmé**, chemin `UserLocalConfigStore > Software > Valve > Steam > apps > <appid>`, champs `Playtime` (minutes) et `LastPlayed` (Unix). Sert de temps de jeu sans clé API. `accountId = SteamID64 − 76561197960265728`.
- `config\loginusers.vdf` : la clé est `timestamp` (minuscules) et `MostRecent` peut être absent : on trie par `MostRecent` puis `timestamp`.
- Cache d'images : beaucoup d'anciens jeux n'ont **pas** de `library_600x900.jpg`, seulement `header.jpg`. Ordre utilisé : fichier local (dossier direct ou sous-dossier haché) → `https://shared.steamstatic.com/store_item_assets/steam/apps/<appid>/<fichier>` (vérifié, l'ancien `cdn.cloudflare.steamstatic.com/steam/apps/...` répond aussi) → jaquette générée. Les URL en 404 sont mémorisées 7 jours (`artwork/misses.json`).
- Sans Steam installé mais avec clé + SteamID64, la bibliothèque possédée est quand même importée (lancement impossible tant que Steam est absent).

## Epic Games (priorité 2)

> Sur la machine de l'utilisateur, Epic n'est **pas installé** (`Data\Manifests` absent) : la source devra simplement apparaître « non installée ». Même règle pour tout launcher : facultatif, jamais bloquant.

- Manifests des jeux installés : `C:\ProgramData\Epic\EpicGamesLauncher\Data\Manifests\*.item` (JSON : `DisplayName`, `AppName`, `InstallLocation`, `LaunchExecutable`, `CatalogNamespace`, `CatalogItemId`).
- Jeux possédés mais non installés : ils demandent l'API Epic avec OAuth. **Hors MVP**, on ne liste que les jeux installés au début.
- Lancement : `com.epicgames.launcher://apps/<CatalogNamespace>%3A<CatalogItemId>%3A<AppName>?action=launch&silent=true`, ou la forme courte avec `<AppName>` seul quand les identifiants de catalogue manquent (non vérifié en réel : Epic absent de la machine de l'utilisateur ; couvert par des tests sur des manifests d'exemple).
- Filtrage : on ignore les add-ons (`MainGameAppName` ≠ `AppName`), les plugins (`AppCategories` sans `games`) et les installations incomplètes (`bIsIncompleteInstall`).
- Temps de jeu : pas de source locale fiable. On utilise le suivi local.
- Succès : non disponibles pour le MVP.

## GOG Galaxy (priorité 2)

- Base locale : `C:\ProgramData\GOG.com\Galaxy\storage\galaxy-2.0.db` (SQLite, journal **WAL**). On copie `galaxy-2.0.db` **avec** `-wal` et `-shm` dans un dossier temporaire avant de l'ouvrir : sans le WAL, les données récentes manquent. L'original n'est jamais ouvert.

**Vérifié sur la machine de l'utilisateur (J3, novembre 2026)** :

- `LibraryReleases.releaseKey` (`gog_<productId>`) liste les jeux possédés **et leurs DLC** (43 entrées dont 33 DLC de The Witcher 3). On écarte les DLC listés dans la pièce `dlcs` du jeu de base (`{"dlcs": ["gog_…", …]}`). Il reste des variantes non listées (`dlc_13_a`, `… [A]`, `4k graphic assets pack`) : importées **masquées**.
- Titre : pièce `title` puis `originalTitle` de `GamePieces` (JSON `{"title": …}`), via `GamePieceTypes.type`. Images : pièce `originalImages` (`verticalCover` en WebP, `background`).
- Temps : `GameTimes.minutesInGame` (compteur de la plateforme, règle de cumul) ; `LastPlayedDates.lastPlayedDate` au format `YYYY-MM-DD HH:MM:SS` (UTC).
- Installation : `InstalledBaseProducts(productId, installationPath)` ; exécutable principal : `PlayTasks` (`isPrimary`) → `PlayTaskLaunchParameters.executablePath`.
- Succès : `UserReleaseAchievements` **n'existe pas**. On lit `Achievements` (apikey, rarity, image) + `UserAchievements` (`isUnlocked`, `unlockTime`) + `LocalizedAchievements` (nom/description par `languageId`, français en priorité via la table `Languages`).
- Client : `HKLM\SOFTWARE\WOW6432Node\GOG.com\GalaxyClient\paths\client` + `clientExecutable`.
- Résultat réel : 10 jeux (dont The Witcher 3 : 3 188 min, 21/78 succès) et 13 variantes de DLC masquées.
- La base peut contenir les jeux des **launchers connectés dans Galaxy** (`steam_`, `epic_`…). **On ne lit que `gog_*`** : les autres viennent de leur propre source, ce qui évite de compter deux fois le même temps.
- Lancement : `"<GalaxyClient.exe>" /command=runGame /gameId=<productId>`, ou l'exe direct depuis `goggame-<id>.info` dans le dossier du jeu.
- Temps et succès : présents dans la base Galaxy.

## Sources secondaires — implémentation J5

Battle.net, EA app, Ubisoft Connect et Riot ne lisent que les **jeux installés**, détectés depuis la liste « Programmes installés » de Windows (clés `Uninstall` de HKLM 64 et 32 bits et de HKCU), en lecture seule. Le temps vient du suivi local de Ludothèque. Chaque source est désactivée par défaut et s'active dans Réglages › Sources ou dans l'assistant d'import.

- **Riot — vérifié sur la machine de l'utilisateur (novembre 2026)** : entrées `Riot Game <produit>.<patchline>` (ex. `Riot Game valorant.live`) ; seule la patchline `live` est retenue. `RiotClientServices.exe` est lu dans `C:\ProgramData\Riot Games\RiotClientInstalls.json`. Lancement : `RiotClientServices.exe --launch-product=<produit> --launch-patchline=live`. Le suivi surveille le processus du jeu (`League of Legends.exe`, VALORANT…), pas le client ni le lobby. Trouvés : League of Legends, VALORANT, Teamfight Tactics, 2XKO.
- **Battle.net — client vérifié sur la machine de l'utilisateur (novembre 2026), sans jeu installé** : la liste des jeux **possédés** n'est pas stockée en local. `C:\ProgramData\Battle.net\Agent\product.db` ne décrit que les produits installés (ici l'agent seul), `%LOCALAPPDATA%\Battle.net\CachedData.db` (SQLite) ne contient que le cache de connexion et de catalogue, et `%APPDATA%\Battle.net\Battle.net.config` ne liste un jeu qu'après son installation. Il n'existe pas non plus d'API publique de bibliothèque. Un jeu Battle.net non installé n'est donc pas détecté. Détection des jeux installés : entrées `Publisher = Blizzard Entertainment` dont la commande de désinstallation contient `--uid=<code>` ; lancement `battlenet://<code>` (code = partie avant le suffixe de langue). Les copies Steam et l'app Battle.net elle-même sont écartées. `product.db` (protobuf) n'est pas lu.
- **EA app — non testé** : entrées `Publisher = Electronic Arts` avec un dossier d'installation ; source considérée comme présente si `C:\ProgramData\EA Desktop` existe.
- **Ubisoft Connect — non testé** : `HKLM\SOFTWARE\WOW6432Node\Ubisoft\Launcher\Installs\<id>\InstallDir`, lancement `uplay://launch/<id>/0`.

## Icônes et succès cachés

- **Succès cachés** : `GetSchemaForGame` renvoie `hidden` (0/1) pour chaque succès Steam ; il est gardé dans `achievements.hidden` (migration 0009). GOG et RetroAchievements n'en ont pas.
- **Icônes** : jamais affichées en lien direct. Elles sont copiées dans `artwork/achievements/` (nom = empreinte FNV-1a de l'URL), en arrière-plan à l'ouverture de l'onglet Succès et après la resynchronisation de fin de session, puis l'événement `achievements-icons-ready` recharge la liste. Hôtes acceptés : `steamcdn-a.akamaihd.net`, `*.steamstatic.com`, `images.gog.com`, `*.gog-statics.com`, `media.retroachievements.org`. Les succès verrouillés sont grisés en CSS : pas besoin des icônes `icongray`.

## Battle.net

- Jeux installés : `C:\ProgramData\Battle.net\Agent\product.db` (protobuf). Solution de repli plus simple : les clés de désinstallation dans le registre (`HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*`, `Publisher = Blizzard Entertainment`).
- Lancement : `battlenet://<ProductCode>` (ex. `Fen` pour Diablo IV, à vérifier), ou `Battle.net.exe --exec="launch <code>"`.
- Pas d'API publique pour le temps ni les succès : suivi local et objectifs perso.

## EA app / Ubisoft Connect / Riot / Xbox

- **EA app** : registre et `C:\ProgramData\EA Desktop\InstallData\` (à vérifier). Suivi local.
- **Ubisoft Connect** : registre `HKLM\SOFTWARE\WOW6432Node\Ubisoft\Launcher\Installs\<id>\InstallDir`. Lancement `uplay://launch/<id>/0`. Suivi local.
- **Riot** : `C:\ProgramData\Riot Games\RiotClientInstalls.json` et `Metadata\<product>\<product>.yaml` (à vérifier). Lancement : `RiotClientServices.exe --launch-product=league_of_legends --launch-patchline=live`. Suivi local.
- **Xbox / Game Pass PC** : jeux UWP/MSIX (`Get-AppxPackage`, dossier `XboxGames`). Les succès demandent l'authentification Microsoft (via OpenXBL ou Xbox Live, **phase tardive**).

## Exécutables locaux

- Ajout manuel (.exe, .lnk résolu vers sa cible, .url pour les URI) et scan de dossiers choisis (`C:\Games`, `D:\Games`…).
- Nom par défaut : `FileDescription` / `ProductName` des métadonnées de version de l'exe, sinon le nom du dossier.
- Écarter les faux positifs (voir SPEC § 4). Choisir l'exe le plus gros ou le plus proche de la racine quand il y en a plusieurs.

## Émulateurs + RetroAchievements

Implémenté en J5 (non testé sur la machine de l'utilisateur : aucun émulateur configuré). Réglages › Émulateurs : exe, modèle d'arguments avec `{rom}`, dossier, extensions, console RA. Le titre vient du nom du fichier sans balises (`(USA)`, `[!]`). Le jeu lancé depuis Ludothèque est retenu pendant 2 h pour départager plusieurs ROMs du même émulateur. Hash RA calculé automatiquement pour les consoles à cartouche (ids 1–7, 11, 15, 25 : MD5 du fichier, sans l'en-tête de copieur NES/SNES, N64 ramené en .z64), puis comparé à `API_GetGameList` ; pour les consoles à disque, l'ID RA se saisit sur la fiche. Progression : `API_GetGameInfoAndUserProgress`.

Notes de départ :


- Configuration : un émulateur (chemin de l'exe + modèle d'arguments, ex. `"{rom}"` ou `-batch -fullscreen "{rom}"`) associé à un dossier de ROMs et à des extensions.
- Chaque ROM devient un `game_source` de type `emulator` (`external_id` = chemin de la ROM).
- RetroAchievements : API Web avec nom d'utilisateur + clé API (`API_GetUserProgress`, `API_GetGameInfoAndUserProgress`). Correspondance des ROMs par hash, selon les règles RA propres à chaque console (à vérifier).

## Services de métadonnées

- **SteamGridDB** (`https://www.steamgriddb.com/api/v2`, Bearer token) : `search/autocomplete/{term}`, `grids/game/{id}?dimensions=600x900`, `heroes/game/{id}`, `logos/game/{id}`. Il existe aussi une correspondance par `steam/{appid}`.
  - **Remplissage automatique** (après chaque synchro et chaque ajout manuel, en tâche de fond, si la clé est enregistrée) : pour chaque jeu visible à qui il manque une jaquette, une image de fond ou un logo (après les images Steam), le jeu SteamGridDB est trouvé par l'id déjà connu, puis `games/steam/{appid}`, puis une recherche dont le résultat n'est retenu que si le titre normalisé est identique (`match_key`). La première image de chaque type manquant est téléchargée. Un jeu incomplet n'est retenté qu'après 7 jours (marqueur `sgdb-auto:<id>` dans `artwork/misses.json`).
- **Steam Store** (public, sans clé), pour la fiche du jeu :
  - `GET https://store.steampowered.com/api/appdetails?appids={appid}&l=french` → `{ "{appid}": { success, data } }`. On lit `short_description` (HTML léger, entités décodées), `screenshots[] { path_thumbnail, path_full }` et `movies[] { name, thumbnail, highlight, hls_h264, dash_h264, dash_av1 }`.
  - **Vérifié en novembre 2026** : les bandes-annonces ne sont plus servies en MP4 (`mp4.max`/`webm` ont disparu, `movie_max.mp4` répond 404). Il ne reste que du HLS (`hls_264_master.m3u8`, segments fMP4, piste audio séparée) et du DASH. Le CDN `video.akamai.steamstatic.com` répond `Access-Control-Allow-Origin: *`. La fiche lit donc le HLS avec **hls.js** (WebView2 n'a pas de HLS natif).
  - Pour un jeu sans source Steam (Epic, GOG…), la page Steam est cherchée avec `GET /api/storesearch/?term={titre}&l=french&cc=CH` et retenue seulement si le titre normalisé (`match_key`) est identique.
  - Les actus du jeu utilisent `ISteamNews/GetNewsForApp/v2` (10 éléments, mêmes filtres que l'accueil).
  - Cache mémoire côté backend : 6 h pour les médias et l'appid trouvé par titre, 30 min pour les actus.
- **IGDB** (Twitch) : complète la fiche quand Steam n'a pas de page pour le jeu, ou pas de bande-annonce, pas d'images ou pas de description. Facultatif, implémenté dans `src-tauri/src/metadata/igdb.rs`.
  - Identifiants : l'ID client et le secret d'une application Twitch, de type « Confidentiel » (seul ce type a un secret ; URL de redirection `https://localhost`, inutilisée). Ils sont saisis dans Réglages › Métadonnées et stockés dans le coffre Windows.
  - Jeton : `POST https://id.twitch.tv/oauth2/token?client_id=…&client_secret=…&grant_type=client_credentials` → `{ access_token, expires_in }`. Il est gardé en mémoire seulement et renouvelé une fois sur un 401.
  - Requête : `POST https://api.igdb.com/v4/games` avec les en-têtes `Client-ID` et `Authorization: Bearer …`, et le corps `search "titre"; fields name,summary,screenshots.image_id,artworks.image_id,videos.video_id,videos.name; limit 10;` (ou `where id = N;` si l'id IGDB est connu). Le jeu n'est retenu que si son titre normalisé (`match_key`) est identique ; son id est alors enregistré dans `games.igdb_id`.
  - Images : `https://images.igdb.com/igdb/image/upload/t_screenshot_med/{image_id}.jpg` (vignette) et `t_1080p` (plein écran). Les captures passent en premier, les artworks complètent.
  - Vidéos : ce sont des ids YouTube, lus dans le lecteur `youtube-nocookie.com` (chargé seulement au clic sur Lecture). Le résumé est en anglais.
  - Seul ce qui manque côté Steam est complété. Tant que les identifiants manquent, la fiche le signale et le résultat n'est pas mis en cache.
  - **Vérifié en novembre 2026** avec de vrais identifiants (test `igdb_live`, ignoré par défaut) : la réponse est un tableau de jeux, et `screenshots`, `artworks` et `videos` sont des tableaux d'objets `{ id, image_id }` / `{ id, video_id, name }`. Plusieurs jeux peuvent porter le même titre (« Hades » : un homonyme obscur sans média passe avant le vrai jeu). Parmi les titres identiques, on garde donc celui qui a le plus de médias, puis le plus de notes (`total_rating_count`).
  - Le lecteur `youtube-nocookie.com` s'affiche bien depuis une origine locale (pas d'erreur 153 avec `referrerpolicy="strict-origin-when-cross-origin"`).

  - **Durées de vie** (`docs/TIME-TO-BEAT.md`) : `POST https://api.igdb.com/v4/game_time_to_beats` avec `fields hastily,normally,completely,count; where game_id = {igdb_id};`. **Vérifié le 8 novembre 2026** avec de vrais identifiants : la réponse est un tableau (vide si IGDB n'a rien) d'objets `{ id, game_id, hastily, normally, completely, count, created_at, updated_at, checksum }`, durées en **secondes**. Correspondance : `hastily` → Histoire, `normally` → Histoire + extras, `completely` → Complétionniste ; `count` = nombre de relevés (souvent faible : 13 pour Hades, 7 pour Diablo IV), affiché « d'après N joueurs ». Le jeu IGDB est celui de `games.igdb_id`, sinon celui au titre identique (le plus noté) ; sans titre identique, jusqu'à 5 candidats sont proposés dans le formulaire « Corriger ». Le lien « Voir sur IGDB » est le champ `url` du jeu (`https://www.igdb.com/games/…`). Cache de 30 jours, y compris quand IGDB n'a rien.

- **HowLongToBeat** : **écarté** (novembre 2026). Pas d'API officielle, et le site refuse tout client honnête : une requête `POST /api/search` avec un user-agent `Ignite/…` reçoit un `403 Unavailable (IFW-E01)` de son pare-feu, avant même la lecture de la requête. Le faire fonctionner demanderait de se faire passer pour un navigateur ou d'extraire une clé du site, ce que nos règles excluent. Les durées viennent donc d'IGDB et de la correction manuelle. La colonne `games.hltb_main_minutes` garde son nom historique.
