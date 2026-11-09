import { useEffect } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { listen } from '@tauri-apps/api/event';
import {
  commands,
  isTauri,
  unwrap,
  type BulkUpdate,
  type GameUpdate,
  type LibraryQuery,
  type SecretKind,
  type SessionInput,
  type SourceKind,
  type TimeToBeatView,
} from './client';
import type { ManualTimeToBeat } from './bindings';

export const keys = {
  library: (q: LibraryQuery) => ['library', q] as const,
  counts: ['counts'] as const,
  game: (id: string) => ['game', id] as const,
  sources: ['sources'] as const,
  steamSettings: ['steam-settings'] as const,
  secrets: ['secrets'] as const,
  tracker: ['tracker'] as const,
  sessions: (gameId: string) => ['sessions', gameId] as const,
  goals: (gameId: string) => ['goals', gameId] as const,
  achievements: (gameId: string) => ['achievements', gameId] as const,
  captures: (gameId: string) => ['captures', gameId] as const,
  captureFolders: ['capture-folders'] as const,
};

/** Everything derived from the library must refresh after a change. */
const LIBRARY_KEYS = [
  ['library'],
  ['counts'],
  ['game'],
  ['sources'],
  ['sessions'],
  ['goals'],
  ['collections'],
];

export function useLibraryList(query: LibraryQuery) {
  return useQuery({
    queryKey: keys.library(query),
    queryFn: () => unwrap(commands.libraryList(query)),
    enabled: isTauri(),
    placeholderData: (prev) => prev,
  });
}

export function useLibraryCounts() {
  return useQuery({
    queryKey: keys.counts,
    queryFn: () => unwrap(commands.libraryCounts()),
    enabled: isTauri(),
  });
}

export function useGame(id: string | null) {
  return useQuery({
    queryKey: keys.game(id ?? ''),
    queryFn: () => unwrap(commands.gameGet(id!)),
    enabled: isTauri() && !!id,
  });
}

/** Steam Store trailers, screenshots and description of a game (cached 6 h backend-side). */
export function useGameMedia(id: string) {
  return useQuery({
    queryKey: ['game-media', id],
    queryFn: () => unwrap(commands.gameStoreMedia(id)),
    enabled: isTauri() && !!id,
    staleTime: 60 * 60_000,
    retry: 1,
  });
}

export function useGameNews(id: string) {
  return useQuery({
    queryKey: ['game-news', id],
    queryFn: () => unwrap(commands.gameNews(id)),
    enabled: isTauri() && !!id,
    staleTime: 30 * 60_000,
    retry: 1,
  });
}

export function useOpenLink() {
  return useMutation({ mutationFn: (url: string) => unwrap(commands.openLink(url)) });
}

/** Time to beat of a game (IGDB, cached 30 days, plus the user's correction). */
export function useTimeToBeat(gameId: string) {
  return useQuery({
    queryKey: ['ttb', gameId],
    queryFn: () => unwrap(commands.timeToBeatGet(gameId)),
    enabled: isTauri(),
    staleTime: 60 * 60_000,
  });
}

export function useTimeToBeatMutations(gameId: string) {
  const client = useQueryClient();
  const done = (view: TimeToBeatView) => {
    client.setQueryData(['ttb', gameId], view);
    // The best "main story" value feeds the library, home and collections.
    for (const queryKey of [['game', gameId], ['library'], ['home']]) {
      void client.invalidateQueries({ queryKey });
    }
  };
  return {
    refresh: useMutation({
      mutationFn: () => unwrap(commands.timeToBeatRefresh(gameId)),
      onSuccess: done,
    }),
    pick: useMutation({
      mutationFn: (igdbId: number) => unwrap(commands.timeToBeatPick(gameId, igdbId)),
      onSuccess: done,
    }),
    setManual: useMutation({
      mutationFn: (values: ManualTimeToBeat | null) =>
        unwrap(commands.timeToBeatSetManual(gameId, values)),
      onSuccess: done,
    }),
  };
}

export function useOpenFolder() {
  return useMutation({
    mutationFn: (sourceId: string) => unwrap(commands.sourceOpenFolder(sourceId)),
  });
}

function useInvalidateLibrary() {
  const client = useQueryClient();
  return () => Promise.all(LIBRARY_KEYS.map((queryKey) => client.invalidateQueries({ queryKey })));
}

export function useUpdateGame(id: string) {
  const client = useQueryClient();
  const invalidate = useInvalidateLibrary();
  return useMutation({
    mutationFn: (update: GameUpdate) => unwrap(commands.gameUpdate(id, update)),
    onSuccess: (detail) => {
      client.setQueryData(keys.game(id), detail);
      void invalidate();
    },
  });
}

export function useAddTag(id: string) {
  const invalidate = useInvalidateLibrary();
  return useMutation({
    mutationFn: (name: string) => unwrap(commands.gameTagAdd(id, name)),
    onSuccess: () => invalidate(),
  });
}

export function useRemoveTag(id: string) {
  const invalidate = useInvalidateLibrary();
  return useMutation({
    mutationFn: (tagId: string) => unwrap(commands.gameTagRemove(id, tagId)),
    onSuccess: () => invalidate(),
  });
}

export function useLaunchGame() {
  return useMutation({
    mutationFn: (gameId: string) => unwrap(commands.launchGame(gameId)),
  });
}

export function useSync() {
  const invalidate = useInvalidateLibrary();
  return useMutation({
    mutationFn: () => unwrap(commands.librarySync()),
    onSettled: () => invalidate(),
  });
}

export function useSourcesStatus() {
  return useQuery({
    queryKey: keys.sources,
    queryFn: () => unwrap(commands.sourcesStatus()),
    enabled: isTauri(),
    // Refresh while a background sync is running.
    refetchInterval: (q) => (q.state.data?.some((s) => s.syncing) ? 1500 : false),
  });
}

export function useSetSourceEnabled() {
  const client = useQueryClient();
  const invalidate = useInvalidateLibrary();
  return useMutation({
    mutationFn: async ({ kind, enabled }: { kind: SourceKind; enabled: boolean }) => {
      await unwrap(commands.sourceSetEnabled(kind, enabled));
      // Import right away when a source is turned on.
      if (enabled) await unwrap(commands.librarySync());
    },
    onSettled: () => {
      void client.invalidateQueries({ queryKey: keys.sources });
      void invalidate();
    },
  });
}

export function useDuplicates() {
  return useQuery({
    queryKey: ['duplicates'],
    queryFn: () => unwrap(commands.duplicatesList()),
    enabled: isTauri(),
  });
}

export function useMergeGames() {
  const client = useQueryClient();
  const invalidate = useInvalidateLibrary();
  return useMutation({
    mutationFn: ({ targetId, otherIds }: { targetId: string; otherIds: string[] }) =>
      unwrap(commands.gameMerge(targetId, otherIds)),
    onSuccess: () => {
      void client.invalidateQueries({ queryKey: ['duplicates'] });
      void invalidate();
    },
  });
}

export function useSplitSource() {
  const client = useQueryClient();
  const invalidate = useInvalidateLibrary();
  return useMutation({
    mutationFn: (sourceId: string) => unwrap(commands.gameSplit(sourceId)),
    onSuccess: () => {
      void client.invalidateQueries({ queryKey: ['duplicates'] });
      void invalidate();
    },
  });
}

export function useCollections() {
  return useQuery({
    queryKey: ['collections'],
    queryFn: () => unwrap(commands.collectionsList()),
    enabled: isTauri(),
  });
}

export function useDeleteCollection() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => unwrap(commands.collectionDelete(id)),
    onSuccess: () => client.invalidateQueries({ queryKey: ['collections'] }),
  });
}

export function useSetCollectionMember() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: ({ id, gameId, member }: { id: string; gameId: string; member: boolean }) =>
      unwrap(commands.collectionSetMember(id, gameId, member)),
    onSuccess: () => {
      void client.invalidateQueries({ queryKey: ['collections'] });
      void client.invalidateQueries({ queryKey: ['library'] });
    },
  });
}

export function useCollectionMembership(gameId: string, ids: string[]) {
  return useQuery({
    queryKey: ['collections', 'members', gameId, ids.length],
    queryFn: async () => new Set(await unwrap(commands.gameCollections(gameId))),
    enabled: isTauri() && ids.length > 0,
  });
}

export function useSearchCheck(text: string) {
  return useQuery({
    queryKey: ['search-check', text],
    queryFn: () => unwrap(commands.searchCheck(text)),
    enabled: isTauri() && text.includes(':'),
    staleTime: Infinity,
  });
}

export function useTrackingSettings() {
  return useQuery({
    queryKey: ['tracking-settings'],
    queryFn: () => commands.trackingSettingsGet(),
    enabled: isTauri(),
  });
}

export function useSetTrackingSettings() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (settings: {
      minSessionMinutes: number;
      idleMinutes: number;
      achievementToasts: boolean;
    }) => unwrap(commands.trackingSettingsSet(settings)),
    onSuccess: () => client.invalidateQueries({ queryKey: ['tracking-settings'] }),
  });
}

export function useSecretsStatus() {
  return useQuery({
    queryKey: keys.secrets,
    queryFn: () => unwrap(commands.secretsStatus()),
    enabled: isTauri(),
  });
}

export function useSteamSettings() {
  return useQuery({
    queryKey: keys.steamSettings,
    queryFn: () => unwrap(commands.steamSettingsGet()),
    enabled: isTauri(),
  });
}

export function useSetSteamId() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (steamId: string | null) => unwrap(commands.steamSettingsSet(steamId)),
    onSuccess: () => client.invalidateQueries({ queryKey: keys.steamSettings }),
  });
}

export function useSetSecret() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: ({ kind, value }: { kind: SecretKind; value: string }) =>
      unwrap(commands.secretsSet(kind, value)),
    onSuccess: () => {
      void client.invalidateQueries({ queryKey: keys.steamSettings });
      void client.invalidateQueries({ queryKey: keys.secrets });
      void client.invalidateQueries({ queryKey: keys.sources });
    },
  });
}

export function useClearSecret() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (kind: SecretKind) => unwrap(commands.secretsClear(kind)),
    onSuccess: () => {
      void client.invalidateQueries({ queryKey: keys.steamSettings });
      void client.invalidateQueries({ queryKey: keys.secrets });
      void client.invalidateQueries({ queryKey: keys.sources });
    },
  });
}

/** Refreshes data whenever the backend reports a change (sync, artwork, sessions). */
export function useLibraryEvents() {
  const client = useQueryClient();
  useEffect(() => {
    if (!isTauri()) return;
    const unlisteners: (() => void)[] = [];
    let cancelled = false;
    const refreshLibrary = () => {
      for (const queryKey of LIBRARY_KEYS) void client.invalidateQueries({ queryKey });
    };
    const refreshTracker = () => {
      void client.invalidateQueries({ queryKey: keys.tracker });
      refreshLibrary();
    };
    const subscriptions: [string, () => void][] = [
      ['library-changed', refreshLibrary],
      ['session-started', refreshTracker],
      ['session-ended', refreshTracker],
    ];
    for (const [event, handler] of subscriptions) {
      void listen(event, handler).then((fn) => {
        if (cancelled) fn();
        else unlisteners.push(fn);
      });
    }
    // New captures or thumbnails: the tab and the open recap show them.
    void listen('captures-changed', () => {
      void client.invalidateQueries({ queryKey: ['captures'] });
      void client.invalidateQueries({ queryKey: ['session-recap'] });
    }).then((fn) => {
      if (cancelled) fn();
      else unlisteners.push(fn);
    });
    // Achievement icons now in the image cache: show them.
    void listen<string>('achievements-icons-ready', ({ payload }) => {
      void client.invalidateQueries({ queryKey: keys.achievements(payload) });
    }).then((fn) => {
      if (cancelled) fn();
      else unlisteners.push(fn);
    });
    // A Goldberg achievement was just unlocked: refresh the list.
    void listen<{ gameId: string }>('achievement-unlocked', ({ payload }) => {
      void client.invalidateQueries({ queryKey: keys.achievements(payload.gameId) });
      void client.invalidateQueries({ queryKey: keys.game(payload.gameId) });
      void client.invalidateQueries({ queryKey: ['goldberg', payload.gameId] });
    }).then((fn) => {
      if (cancelled) fn();
      else unlisteners.push(fn);
    });
    return () => {
      cancelled = true;
      unlisteners.forEach((fn) => fn());
    };
  }, [client]);
}

/** Games being played now; polled while a session is active. */
export function useTrackerActive() {
  return useQuery({
    queryKey: keys.tracker,
    queryFn: () => commands.trackerActive(),
    enabled: isTauri(),
    refetchInterval: (q) => ((q.state.data?.length ?? 0) > 0 ? 15_000 : 60_000),
  });
}

export function useSessions(gameId: string) {
  return useQuery({
    queryKey: keys.sessions(gameId),
    queryFn: () => unwrap(commands.sessionsList(gameId)),
    enabled: isTauri(),
  });
}

export function useSaveSession() {
  const invalidate = useInvalidateLibrary();
  return useMutation({
    mutationFn: (input: SessionInput) => unwrap(commands.sessionSave(input)),
    onSuccess: () => invalidate(),
  });
}

export function useDeleteSession() {
  const invalidate = useInvalidateLibrary();
  return useMutation({
    mutationFn: (id: string) => unwrap(commands.sessionDelete(id)),
    onSuccess: () => invalidate(),
  });
}

export function useGoals(gameId: string) {
  return useQuery({
    queryKey: keys.goals(gameId),
    queryFn: () => unwrap(commands.goalsList(gameId)),
    enabled: isTauri(),
  });
}

export function useGoalMutations(gameId: string) {
  const invalidate = useInvalidateLibrary();
  const opts = { onSuccess: () => invalidate() };
  return {
    add: useMutation({
      mutationFn: (title: string) => unwrap(commands.goalAdd(gameId, title)),
      ...opts,
    }),
    toggle: useMutation({
      mutationFn: ({ id, done }: { id: string; done: boolean }) =>
        unwrap(commands.goalToggle(id, done)),
      ...opts,
    }),
    rename: useMutation({
      mutationFn: ({ id, title }: { id: string; title: string }) =>
        unwrap(commands.goalRename(id, title)),
      ...opts,
    }),
    remove: useMutation({ mutationFn: (id: string) => unwrap(commands.goalDelete(id)), ...opts }),
  };
}

export function useAchievements(gameId: string, enabled = true) {
  return useQuery({
    queryKey: keys.achievements(gameId),
    queryFn: () => unwrap(commands.achievementsGet(gameId, false)),
    enabled: isTauri() && enabled,
    staleTime: 5 * 60_000,
  });
}

/** Goldberg setup state of a local game. */
export function useGoldbergStatus(gameId: string, enabled = true) {
  return useQuery({
    queryKey: ['goldberg', gameId],
    queryFn: () => unwrap(commands.goldbergStatus(gameId)),
    enabled: isTauri() && enabled,
  });
}

/** Installs Goldberg and writes the definitions for this game. */
export function useGoldbergSetup(gameId: string) {
  const client = useQueryClient();
  const invalidate = useInvalidateLibrary();
  return useMutation({
    mutationFn: ({ appid, force }: { appid: number; force: boolean }) =>
      unwrap(commands.goldbergSetup(gameId, appid, force)),
    onSuccess: (data) => {
      client.setQueryData(['goldberg', gameId], data);
      void client.invalidateQueries({ queryKey: keys.achievements(gameId) });
      void invalidate();
    },
  });
}

export function useGoldbergClearAppid(gameId: string) {
  const client = useQueryClient();
  return useMutation({
    mutationFn: () => unwrap(commands.goldbergClearAppid(gameId)),
    onSuccess: (data) => {
      client.setQueryData(['goldberg', gameId], data);
      void client.invalidateQueries({ queryKey: keys.achievements(gameId) });
    },
  });
}

export function useGoldbergOpenSaves(gameId: string) {
  return useMutation({ mutationFn: () => unwrap(commands.goldbergOpenSaves(gameId)) });
}

/** Scans the usual Goldberg save folders for achievements already unlocked. */
export function useGoldbergFindSaves(gameId: string) {
  const client = useQueryClient();
  return useMutation({
    mutationFn: () => unwrap(commands.goldbergFindSaves(gameId)),
    onSuccess: () => {
      // Newly found unlocks show in the list right away.
      void client.invalidateQueries({ queryKey: keys.achievements(gameId) });
    },
  });
}

/** Sends a test notification through the achievement-toast path. */
export function useGoldbergTestToast() {
  return useMutation({
    mutationFn: ({ title, body }: { title: string; body: string }) =>
      unwrap(commands.goldbergTestToast(title, body)),
  });
}

export function useRefreshAchievements(gameId: string) {
  const client = useQueryClient();
  const invalidate = useInvalidateLibrary();
  return useMutation({
    mutationFn: () => unwrap(commands.achievementsGet(gameId, true)),
    onSuccess: (data) => {
      client.setQueryData(keys.achievements(gameId), data);
      void invalidate();
    },
  });
}

/** Status, hidden or favorite flag for several games (multi-selection). */
export function useBulkUpdate() {
  const invalidate = useInvalidateLibrary();
  return useMutation({
    mutationFn: ({ ids, update }: { ids: string[]; update: BulkUpdate }) =>
      unwrap(commands.gamesBulkUpdate(ids, update)),
    onSettled: () => void invalidate(),
  });
}

/** Deletes executable games (the files on disk are kept). */
export function useDeleteGames() {
  const invalidate = useInvalidateLibrary();
  return useMutation({
    mutationFn: (ids: string[]) => unwrap(commands.gamesDelete(ids)),
    onSettled: () => invalidate(),
  });
}

/** Adds games to a manual collection; creates it first when `newName` is set. */
export function useAddToCollection() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: async ({
      id,
      newName,
      gameIds,
    }: {
      id?: string;
      newName?: string;
      gameIds: string[];
    }) => {
      const target = newName
        ? (await unwrap(commands.collectionCreate(newName, 'manual', null))).id
        : id!;
      await unwrap(commands.collectionAddGames(target, gameIds));
      return target;
    },
    onSettled: () => {
      void client.invalidateQueries({ queryKey: ['collections'] });
      void client.invalidateQueries({ queryKey: ['library'] });
    },
  });
}

export function useCaptures(gameId: string) {
  return useQuery({
    queryKey: keys.captures(gameId),
    queryFn: () => unwrap(commands.capturesList(gameId)),
    enabled: isTauri() && !!gameId,
  });
}

/** Rescans the capture folders in the background (`captures-changed` follows). */
export function useCapturesScan(gameId: string, enabled: boolean) {
  useEffect(() => {
    if (isTauri() && enabled && gameId) void commands.capturesScan();
  }, [gameId, enabled]);
}

export function useCaptureFolders() {
  return useQuery({
    queryKey: keys.captureFolders,
    queryFn: () => unwrap(commands.captureFoldersList()),
    enabled: isTauri(),
  });
}

export function useCaptureActions(gameId: string) {
  const client = useQueryClient();
  const refreshGame = () => {
    for (const queryKey of [['library'], ['game'], ['home']]) {
      void client.invalidateQueries({ queryKey });
    }
  };
  const refreshFolders = () => {
    void client.invalidateQueries({ queryKey: keys.captureFolders });
    void client.invalidateQueries({ queryKey: ['captures'] });
  };
  return {
    useAsHero: useMutation({
      mutationFn: (id: string) => unwrap(commands.captureUseAsHero(id)),
      onSuccess: refreshGame,
    }),
    useStoreImage: useMutation({
      mutationFn: (url: string) => unwrap(commands.artworkUseStoreImage(gameId, url)),
      onSuccess: (detail) => {
        client.setQueryData(keys.game(gameId), detail);
        refreshGame();
      },
    }),
    copy: useMutation({ mutationFn: (id: string) => unwrap(commands.captureCopy(id)) }),
    reveal: useMutation({ mutationFn: (id: string) => unwrap(commands.captureReveal(id)) }),
    open: useMutation({ mutationFn: (id: string) => unwrap(commands.captureOpen(id)) }),
    openFolder: useMutation({
      mutationFn: (id: string) => unwrap(commands.captureOpenFolder(id)),
    }),
    caption: useMutation({
      mutationFn: ({ id, caption }: { id: string; caption: string | null }) =>
        unwrap(commands.captureSetCaption(id, caption)),
      onSuccess: () => void client.invalidateQueries({ queryKey: keys.captures(gameId) }),
    }),
    addFolder: useMutation({
      mutationFn: ({ path, forGame }: { path: string; forGame: string | null }) =>
        unwrap(commands.captureFolderAdd(path, forGame)),
      onSuccess: refreshFolders,
    }),
    removeFolder: useMutation({
      mutationFn: (path: string) => unwrap(commands.captureFolderRemove(path)),
      onSuccess: refreshFolders,
    }),
  };
}
