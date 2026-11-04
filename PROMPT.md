Salut ! On démarre le développement de **Ludothèque**, un launcher de jeux PC pour Windows qui regroupe dans une seule bibliothèque les jeux Steam et non-Steam, avec lancement, suivi du temps de jeu, succès et objectifs, et statistiques.

Tout le contexte est dans ce dossier :
- `CLAUDE.md` : stack et règles du projet
- `docs/SPEC.md`, `docs/ARCHITECTURE.md`, `docs/INTEGRATIONS.md`, `docs/DESIGN.md`, `docs/ROADMAP.md`
- `design/tokens.css` (thème « Braise ») et `design/mockups/` (maquettes de référence)

Ce que je te demande :
1. Lis tous ces documents, puis résume en 10 lignes maximum ce que tu as compris, avec les points flous ou risqués que tu vois.
2. Propose le plan détaillé du **jalon J0** (fichiers à créer, dépendances avec leurs versions), puis réalise-le.
3. Arrête-toi à la fin de J0 : dis-moi comment lancer l'app et ce que je dois vérifier. On passera à J1 ensuite.

Contraintes :
- Mon PC est sous Windows 11. Steam est installé dans `C:\Program Files (x86)\Steam`.
- Ne me demande jamais de coller une clé API dans le chat : prévois un champ dans les réglages de l'app, stocké dans le coffre de Windows.
- Quand un format de fichier marqué « à vérifier » ne correspond pas à la réalité, adapte le code et mets la doc à jour.
