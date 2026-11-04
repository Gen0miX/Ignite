# Maquettes de référence

Ces fichiers `.dc.html` sont les sources des maquettes du canvas de design. Ce ne sont **pas** des pages à réutiliser telles quelles :

- elles dépendent du runtime de l'éditeur (`support.js`, balises `<x-dc>`, `<sc-for>`, `<sc-if>`, valeurs `{{…}}` calculées dans `renderVals()`) et ne s'ouvrent pas seules dans un navigateur ;
- les images pointent vers `/_blob/...`, des fichiers hébergés par le canvas. Dans l'app, elles viennent du cache Steam local et de SteamGridDB ;
- les chiffres (temps de jeu, compteurs) et les jeux non-Steam sont des **données d'exemple**.

Elles servent à reprendre **la structure, les espacements, les tailles et les textes** de chaque écran. Les couleurs en dur sont celles de la palette Lime. Convertis-les avec la table de correspondance de `docs/DESIGN.md`, à l'exception de `Multi-Bibliotheque.dc.html`, qui contient déjà les 6 palettes, dont « Braise ».

| Fichier | Écran |
|---|---|
| `Multi-Bibliotheque.dc.html` | Bibliothèque unifiée (écran principal) |
| `Multi-Ajouter.dc.html` | Modale Ajouter un jeu |
| `Multi-Import.dc.html` | Parcours d'import |
| `Multi-Fiche.dc.html` | Fiche d'un jeu (non-Steam) |
| `Multi-Stats.dc.html` | Statistiques |
| `Multi-Sources.dc.html` | Réglages › Sources & suivi |
| `Fiche.dc.html` | Fiche d'un jeu Steam, direction A : contenu à reprendre (succès proches, amis, actus filtrées) dans le style B |
| `Main.dc.html` | Accueil, direction A (phase tardive) |
| `Canape.dc.html` | Mode Canapé (phase tardive) |
