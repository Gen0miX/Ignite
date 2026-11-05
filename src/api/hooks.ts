import { useEffect } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { listen } from '@tauri-apps/api/event';
import {
  commands,
  isTauri,
  unwrap,
  type GameUpdate,
  type LibraryQuery,
  type SecretKind,
  type SessionInput,
  type SourceKind,
} from './client';

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
