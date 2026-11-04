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
- Lancement : `com.epicgames.launcher://apps/<CatalogNamespace>%3A<CatalogItemId>%3A<AppName>?action=launch&silent=true` (ou la forme courte avec `<AppName>` seul, à vérifier).
- Temps de jeu : pas de source locale fiable. On utilise le suivi local.
- Succès : non disponibles pour le MVP.

## GOG Galaxy (priorité 2)

- Base locale : `C:\ProgramData\GOG.com\Galaxy\storage\galaxy-2.0.db` (SQLite). **Copie le fichier avant de l'ouvrir**, il peut être verrouillé. Tables utiles (à vérifier) : `InstalledBaseProducts`, `LibraryReleases`, `GamePieces` (titre, images), `GameTimes` (temps en minutes), `LastPlayedDates`, `UserReleaseAchievements` / `Achievements`.
- La base contient aussi les jeux des **launchers que l'utilisateur a connectés dans Galaxy**, avec un préfixe `releaseKey` comme `steam_`, `epic_`, `battlenet_`… On peut s'en servir de source secondaire, mais on dédoublonne avec nos propres sources.
- Lancement : `"<GalaxyClient.exe>" /command=runGame /gameId=<productId>`, ou l'exe direct depuis `goggame-<id>.info` dans le dossier du jeu.
- Temps et succès : présents dans la base Galaxy.

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

## Émulateurs + RetroAchievements (phase tardive)

- Configuration : un émulateur (chemin de l'exe + modèle d'arguments, ex. `"{rom}"` ou `-batch -fullscreen "{rom}"`) associé à un dossier de ROMs et à des extensions.
- Chaque ROM devient un `game_source` de type `emulator` (`external_id` = chemin de la ROM).
- RetroAchievements : API Web avec nom d'utilisateur + clé API (`API_GetUserProgress`, `API_GetGameInfoAndUserProgress`). Correspondance des ROMs par hash, selon les règles RA propres à chaque console (à vérifier).

## Services de métadonnées

- **SteamGridDB** (`https://www.steamgriddb.com/api/v2`, Bearer token) : `search/autocomplete/{term}`, `grids/game/{id}?dimensions=600x900`, `heroes/game/{id}`, `logos/game/{id}`. Il existe aussi une correspondance par `steam/{appid}`.
- **IGDB** (Twitch OAuth client credentials, à faire côté backend) : `POST /v4/games` avec la requête `fields name,genres.name,themes.name,cover.image_id; search "…";`.
- **HowLongToBeat** : pas d'API officielle. Facultatif, désactivé par défaut, et isolé dans un module remplaçable.
