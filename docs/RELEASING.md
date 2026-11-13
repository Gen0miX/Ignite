# Publier une version d'Ignite

Les releases sont construites par GitHub Actions ([`.github/workflows/release.yml`](../.github/workflows/release.yml)) quand on pousse un tag `vX.Y.Z`. Le workflow compile l'installeur NSIS, le **signe pour le module de mise à jour**, et crée une release **en brouillon** avec :

- `Ignite_X.Y.Z_x64-setup.exe` : l'installeur à télécharger ;
- `Ignite_X.Y.Z_x64-setup.exe.sig` : sa signature ;
- `latest.json` : lu par les Ignite déjà installés pour proposer la mise à jour.

## Une seule fois : les secrets GitHub

Le module de mise à jour n'installe que des fichiers signés avec la clé privée dont la clé publique est dans `src-tauri/tauri.conf.json` (`plugins.updater.pubkey`).

La paire a été générée avec `pnpm tauri signer generate` et se trouve **hors du dépôt** :

- `~/.tauri/ignite.key` : clé privée (à ne jamais publier) ;
- `~/.tauri/ignite.key.password` : son mot de passe ;
- `~/.tauri/ignite.key.pub` : clé publique (déjà dans la config).

Dans GitHub › **Settings › Secrets and variables › Actions › New repository secret**, crée :

| Nom | Valeur |
|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | le contenu de `~/.tauri/ignite.key` |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | le contenu de `~/.tauri/ignite.key.password` |

> [!WARNING]
> Sauvegarde la clé privée et son mot de passe (gestionnaire de mots de passe). Sans eux, les versions déjà installées ne pourront plus recevoir de mise à jour automatique : il faudrait générer une nouvelle paire et réinstaller à la main.

## À chaque version

1. Mets à jour [`CHANGELOG.md`](../CHANGELOG.md) : renomme « Non publié » en `[X.Y.Z] - AAAA-MM-JJ`.
2. Change la version aux trois endroits (elles doivent être identiques) :
   - `package.json` › `version`
   - `src-tauri/Cargo.toml` › `version` (puis `cargo check` pour mettre à jour `Cargo.lock`)
   - `src-tauri/tauri.conf.json` › `version`
3. Commit : `chore: version X.Y.Z`.
4. Tag et push :

   ```bash
   git tag vX.Y.Z
   git push origin main --tags
   ```

   Le workflow vérifie que le tag correspond à la version de `tauri.conf.json`.
5. Dans **Actions**, attends la fin du workflow « Release », puis ouvre la release en brouillon dans **Releases**. Vérifie les trois fichiers, colle les notes du CHANGELOG si tu veux, et clique **Publish release**.

Tant que la release est en brouillon, `latest.json` n'est pas visible : aucun Ignite installé ne propose la mise à jour. Une fois publiée, ils la proposent au démarrage suivant.

## Construire en local

`createUpdaterArtifacts` est activé, donc `pnpm tauri build` a besoin de la clé privée :

```bash
TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.tauri/ignite.key)" TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$(cat ~/.tauri/ignite.key.password)" pnpm tauri build
```

L'installeur sort dans `src-tauri/target/release/bundle/nsis/`.

## Vérifier qu'une mise à jour fonctionne

Installe la version précédente, publie la nouvelle, puis lance Ignite : après quelques secondes, le bandeau « Ignite X.Y.Z est disponible » apparaît en bas à droite. « Installer et redémarrer » télécharge l'installeur, vérifie sa signature, l'exécute en mode passif et relance Ignite.
