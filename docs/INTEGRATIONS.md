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
- `ISteamUserStats/GetGlobalAchievementPercentagesForApp/v2?gameid=` : la rareté des succès.
- `ISteamUser/GetPlayerSummaries/v2` et `GetFriendList/v1` : les amis (pour plus tard).
- `ISteamNews/GetNewsForApp/v2` : les actus de chaque jeu (à filtrer, voir SPEC § 4).
- Le SteamID64 vient de `config\loginusers.vdf`, ou bien l'utilisateur le saisit.
- Limiter les appels (~1 requête/s, cache 15 min). Le profil doit être public, ou la clé doit appartenir au compte.

**Lancement** : `steam://rungameid/<appid>`. **Processus à suivre** : les exe sous `<library>\steamapps\common\<installdir>\`.

## Epic Games (priorité 2)

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
