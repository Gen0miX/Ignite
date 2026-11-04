// All user-facing text. The UI is French; keep code identifiers in English.

export const fr = {
  app: {
    name: 'Ludothèque',
  },
  nav: {
    label: 'Bibliothèque',
    allGames: 'Tous les jeux',
    installed: 'Installés',
    favorites: 'Favoris',
    stats: 'Statistiques',
  },
  sidebar: {
    sources: 'Sources',
    noSources: 'Aucune source connectée',
    manageSources: 'Gérer les sources',
    status: 'Statut',
    importGames: 'Importer des jeux',
  },
  status: {
    playing: 'En cours',
    done: 'Terminés',
    todo: 'À faire',
    dropped: 'Abandonnés',
  },
  library: {
    title: 'Tous les jeux',
    installedTitle: 'Installés',
    favoritesTitle: 'Favoris',
    count: (n: number) => `${n} ${n > 1 ? 'jeux' : 'jeu'}`,
    emptyTitle: 'Ta bibliothèque est vide',
    emptyBody: 'Importe tes jeux Steam et ceux des autres launchers pour commencer.',
    emptyAction: 'Importer des jeux',
  },
  game: {
    title: 'Fiche du jeu',
    notFound: 'Ce jeu est introuvable.',
    backToLibrary: 'Retour à la bibliothèque',
  },
  stats: {
    title: 'Statistiques',
    empty: 'Les statistiques apparaîtront après tes premières sessions de jeu.',
  },
  settings: {
    sourcesTitle: 'Sources & suivi',
    sourcesEmpty: 'La connexion des sources arrive bientôt.',
    about: 'À propos',
    version: 'Version',
    database: 'Base de données',
    dataDir: 'Dossier des données',
    backendUnavailable: 'Backend indisponible (lancer avec pnpm tauri dev).',
    loading: 'Chargement…',
  },
  import: {
    title: 'Importer des jeux',
    empty: "L'import depuis Steam et les autres launchers arrive bientôt.",
  },
  notFound: {
    title: 'Page introuvable',
    action: 'Retour à la bibliothèque',
  },
} as const;
